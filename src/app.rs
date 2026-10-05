use crate::config::{self, AgentDef, Config, Fallback};
use crate::demo::Demo;
use crate::deps;
use crate::git::{self, Snapshot};
use crate::guard::{self, Verdict};
use crate::integrity;
use crate::limits::{self, Service};
use crate::mail;
use crate::roles;
use crate::savings;
use crate::secrets;
use crate::stats;
use crate::theme::Theme;
use crate::vendors::{self, AgentEvent, short};
use crate::work;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::collections::{HashMap, VecDeque};
use std::io::Write;
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::mpsc::UnboundedSender;

pub enum Msg {
    Key(KeyEvent),
    Redraw,
    /// agent index, run id (events from a run that was replaced are dropped), event
    Agent(usize, u64, AgentEvent),
    /// customs' free check: (ecosystem, name) of new packages, and the ones the registry does not know
    Deps {
        hash: String,
        added: Vec<(&'static str, String)>,
        missing: Vec<(&'static str, String)>,
    },
    /// A model tried once before an approved switch: Ok, or why it cannot run.
    Probe {
        agent: String,
        vendor: String,
        model: String,
        result: Result<(), String>,
    },
}

/// Where the owner's current task is: anchor writes criteria, the boss works, anchor checks.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Stage {
    Criteria,
    Building,
    Verifying,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Status {
    Idle,
    Queued,
    Running,
    Searching,
    Asking,
    Done,
    Failed,
}

impl Status {
    pub fn busy(self) -> bool {
        matches!(self, Status::Running | Status::Searching)
    }
}

pub struct Agent {
    pub def: AgentDef,
    pub status: Status,
    pub now: String,
    pub output: VecDeque<String>,
    pub tokens_in: u64,
    pub tokens_out: u64,
    pub context: u64,
    pub tests: Option<(u32, u32)>,
    pub task: String,
    /// When the current run started and the token total at that moment.
    /// Start of the current run: when, and tokens in, tokens out and tool calls at that moment.
    run: Option<(Instant, u64, u64, u64)>,
    pub tools: u64,
    /// Tasks waiting for this agent's current run to end (new commits for the always-on agents).
    queue: VecDeque<String>,
    /// The folder its current run works in: its worktree, or the project.
    pub dir: std::path::PathBuf,
    /// Its REPORT lines from the current run, handed on when another agent needs them.
    last_report: String,
    /// The configured model while the agent covers a used-up limit on a fallback.
    pub primary: Option<AgentDef>,
    run_id: u64,
    handle: Option<tokio::task::AbortHandle>,
    /// The task as it was given, and what was done on it so far, for handing it over.
    goal: String,
    journal: Vec<String>,
    /// Every model it may use is out of limits; it resumes when one resets.
    waiting: bool,
    return_pending: bool,
    /// Tokens spent on earlier vendors before a model switch: (vendor, tokens).
    carried: Vec<(String, u64)>,
    /// `done` and `note` cards for it, read at the start of its next run.
    mailbox: Vec<String>,
    /// Messages waiting for this agent's current run to end.
    inbox: VecDeque<u32>,
    /// When a message last landed here, and its kind, so the card can light up.
    pub flash: Option<(Instant, String)>,
}

impl Agent {
    fn new(def: AgentDef) -> Self {
        Self {
            def,
            status: Status::Idle,
            now: String::new(),
            output: VecDeque::new(),
            tokens_in: 0,
            tokens_out: 0,
            context: 0,
            tests: None,
            task: String::new(),
            run: None,
            tools: 0,
            queue: VecDeque::new(),
            last_report: String::new(),
            dir: std::path::PathBuf::new(),
            primary: None,
            run_id: 0,
            handle: None,
            goal: String::new(),
            journal: vec![],
            waiting: false,
            return_pending: false,
            carried: vec![],
            inbox: VecDeque::new(),
            mailbox: vec![],
            flash: None,
        }
    }

    pub fn tokens(&self) -> u64 {
        self.tokens_in + self.tokens_out
    }

    /// Tokens this agent spent on one vendor, across model switches.
    pub fn tokens_on(&self, vendor: &str) -> u64 {
        let before: u64 = self.carried.iter().map(|(_, t)| t).sum();
        let earlier: u64 = self
            .carried
            .iter()
            .filter(|(v, _)| v == vendor)
            .map(|(_, t)| t)
            .sum();
        earlier
            + if self.def.vendor == vendor {
                self.tokens() - before
            } else {
                0
            }
    }

    /// Close the current run and return its numbers for the record.
    fn finish_run(&mut self, ok: bool) -> Option<stats::Run> {
        let (t0, i0, o0, c0) = self.run.take()?;
        Some(stats::Run {
            at: now_unix(),
            ok,
            input: self.tokens_in - i0,
            output: self.tokens_out - o0,
            tools: self.tools - c0,
            secs: t0.elapsed().as_secs(),
            ..Default::default()
        })
    }

    /// Book the tokens spent so far on the current vendor before switching away from it.
    fn carry(&mut self) {
        let before: u64 = self.carried.iter().map(|(_, t)| t).sum();
        let now = self.tokens() - before;
        self.carried.push((self.def.vendor.clone(), now));
    }

    pub fn say(&mut self, line: impl Into<String>) {
        self.output.push_back(line.into());
        while self.output.len() > 3 {
            self.output.pop_front();
        }
    }
}

pub struct Question {
    pub from: String,
    pub task: String,
    pub text: String,
    pub options: Vec<String>,
    pub chosen: Option<usize>,
    /// Set when the guard asked: the command waiting for a yes or no.
    pub guard_cmd: Option<String>,
    /// Set when the scout proposes a model switch: approving applies it.
    pub change: Option<Change>,
    /// Set for a scanner hit: the second option releases this held commit.
    pub release: Option<String>,
    pub asked: Instant,
}

pub struct Change {
    pub agent: String,
    pub vendor: String,
    pub model: String,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Scope {
    Project,
    All,
    Once,
}

pub struct Remembered {
    pub text: String,
    pub scope: Scope,
}

pub struct Web {
    pub who: String,
    pub query: String,
    pub sources: Vec<String>,
    pub learned: Option<String>,
    pub live: bool,
}

pub struct GuardEvent {
    pub at: String,
    pub who: String,
    pub cmd: String,
    pub verdict: Verdict,
    pub why: String,
}

pub struct LogLine {
    pub at: String,
    pub who: String,
    pub text: String,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Module {
    Team,
    Questions,
    Git,
    Web,
    Guard,
    Log,
    Usage,
}

impl Module {
    pub fn from_name(s: &str) -> Option<Self> {
        Some(match s {
            "team" => Module::Team,
            "questions" => Module::Questions,
            "git" => Module::Git,
            "web" => Module::Web,
            "guard" => Module::Guard,
            "log" => Module::Log,
            "usage" => Module::Usage,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Module::Team => "team",
            Module::Questions => "questions",
            Module::Git => "git",
            Module::Web => "web",
            Module::Guard => "guard",
            Module::Log => "log",
            Module::Usage => "usage",
        }
    }

    fn from_key(c: char) -> Option<Self> {
        Some(match c {
            't' => Module::Team,
            'q' => Module::Questions,
            'g' => Module::Git,
            'w' => Module::Web,
            'x' => Module::Guard,
            'l' => Module::Log,
            'u' => Module::Usage,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum Focus {
    None,
    Questions,
    Input,
}

pub struct App {
    pub cfg: Config,
    pub theme: Theme,
    cfg_mtime: Option<SystemTime>,
    pub demo: Option<Demo>,
    /// Set once at start. `demo` itself is taken out while the script runs, so never test that.
    pub demo_mode: bool,
    pub agents: Vec<Agent>,
    pub questions: Vec<Question>,
    pub q_sel: usize,
    pub remembered: Vec<Remembered>,
    pub web: Vec<Web>,
    pub guard_events: Vec<GuardEvent>,
    pub log: VecDeque<LogLine>,
    pub git: Option<Snapshot>,
    pub limits: Vec<Service>,
    /// Tokens per vendor, sampled every two seconds, for the sparklines.
    pub spark: HashMap<String, VecDeque<u64>>,
    last_totals: HashMap<String, u64>,
    pub focus: Focus,
    pub input: String,
    pub task: String,
    pub plan: Option<(u32, u32)>,
    pub zoom: Option<Module>,
    pub tab: Module,
    pub compact: bool,
    pub toast: Option<(String, String, Instant)>,
    pub frame: usize,
    pub started: Instant,
    pub cwd: String,
    timers: [Instant; 4],
    pub quit: bool,
    /// vendor -> unix time its used-up limit resets
    pub blocked: HashMap<String, i64>,
    /// commit -> (why, who): commits ripple holds back from merging
    pub holds: Vec<(String, String, String)>,
    /// The bursar's savings rules (savings.json).
    pub savings: Vec<savings::Rule>,
    /// Tokens per run this week against last week, "-12%".
    pub burn: Option<String>,
    jobs_started: bool,
    /// Commits the referee has been asked to verify a claim about.
    verified: std::collections::HashSet<String>,
    /// When the owner's current task started and finished, and how often orda had to wake an idle boss.
    pub task_started: Option<Instant>,
    pub task_done: Option<Instant>,
    nudges: u8,
    quiet_since: Option<Instant>,
    /// The first message id of anchor's final check, to see what it sent.
    verify_from: u32,
    /// commit -> (its branch, agents still checking it); the boss is woken when it empties
    pub checks: HashMap<String, (String, Vec<String>)>,
    /// The owner's current task and how far it is.
    pub flow: Option<(Stage, String)>,
    /// Commits already handed to the always-on agents and the scanner.
    seen: std::collections::HashSet<String>,
    pub messages: Vec<mail::Message>,
    pub flights: Vec<mail::Flight>,
    /// Copies of every message, for the boss's next run.
    digest: Vec<String>,
    tx: UnboundedSender<Msg>,
}

const SECOND: Duration = Duration::from_secs(1);
/// How often orda wakes an idle boss before it calls the task stalled.
const MAX_NUDGES: u8 = 3;
/// How long nothing must run before orda wakes the boss.
const QUIET: Duration = Duration::from_secs(20);
const AUDIT_TASK: &str = "Run your weekly audit of the whole project.";
/// How long a card stays lit after a message lands on it.
pub const FLASH: Duration = Duration::from_millis(1600);
/// The scout scans again when orda starts and its last scan is older than this.
const SCOUT_EVERY: i64 = 3 * 86400;
/// aegis reads the whole project again when its last audit is older than this.
const AUDIT_EVERY: i64 = 7 * 86400;

impl App {
    pub fn new(demo: bool, tx: UnboundedSender<Msg>) -> Self {
        let (cfg, err) = config::load();
        let cwd = std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        let now = Instant::now();
        let mut app = Self {
            theme: Theme::build(&cfg.theme, &cfg.colors),
            agents: cfg.agents.iter().cloned().map(Agent::new).collect(),
            cfg,
            cfg_mtime: config::modified(),
            demo: None,
            demo_mode: demo,
            questions: vec![],
            q_sel: 0,
            remembered: vec![],
            web: vec![],
            guard_events: vec![],
            log: VecDeque::new(),
            git: None,
            limits: limits::load(),
            spark: HashMap::new(),
            last_totals: HashMap::new(),
            focus: Focus::None,
            input: String::new(),
            task: String::new(),
            plan: None,
            zoom: None,
            tab: Module::Team,
            compact: false,
            toast: None,
            frame: 0,
            started: now,
            cwd,
            timers: [now; 4],
            quit: false,
            blocked: HashMap::new(),
            holds: vec![],
            savings: vec![],
            burn: None,
            seen: Default::default(),
            flow: None,
            jobs_started: false,
            verified: Default::default(),
            task_started: None,
            task_done: None,
            nudges: 0,
            quiet_since: None,
            verify_from: 0,
            checks: HashMap::new(),
            messages: vec![],
            flights: vec![],
            digest: vec![],
            tx,
        };
        if let Some(e) = err {
            app.note("orda", e);
        }
        if demo {
            app.demo = Some(Demo::seed(&mut app));
        } else {
            app.git = git::snapshot(&app.cwd);
            // only commits made from now on are new
            app.seen = git::recent_all(&app.cwd).into_iter().map(|c| c.0).collect();
            app.remembered = load_answers(&app.cwd);
            app.savings = savings::load();
            app.burn = stats::trend(&stats::runs(), now_unix());
            for a in app.agents.iter_mut().filter(|a| a.def.always_on) {
                a.status = Status::Queued;
                a.now = "waits for the first commit".into();
            }
            app.note(
                "orda",
                format!(
                    "ready in {}. press i and tell orda what to build",
                    tilde(&app.cwd)
                ),
            );
        }
        app
    }

    pub fn note(&mut self, who: &str, text: impl Into<String>) {
        let text = text.into();
        if !self.demo_mode {
            log_line(&self.cwd, who, &text);
        }
        self.log.push_back(LogLine {
            at: clock(),
            who: who.into(),
            text,
        });
        while self.log.len() > 200 {
            self.log.pop_front();
        }
    }

    pub fn ask(&mut self, q: Question) {
        let head = format!("new question from {}", q.from);
        self.note(&q.from.clone(), format!("asked you: {}", q.text));
        if self.cfg.notify.desktop {
            let _ = std::process::Command::new("notify-send")
                .args(["-a", "orda", &format!("orda: {head}"), &q.text])
                .spawn();
        }
        self.toast = Some((head, q.text.clone(), Instant::now()));
        self.questions.push(q);
    }

    /// Modules in config order, used for tabs.
    pub fn modules(&self) -> Vec<Module> {
        let l = &self.cfg.layout;
        let mut out: Vec<Module> = l
            .left
            .iter()
            .chain(&l.right)
            .chain(&l.bottom)
            .filter_map(|s| Module::from_name(s))
            .collect();
        out.dedup();
        out
    }

    pub fn agent_index(&self, name: &str) -> Option<usize> {
        self.agents.iter().position(|a| a.def.name == name)
    }

    pub fn accent_of(&self, name: &str) -> ratatui::style::Color {
        match self.agents.iter().find(|a| a.def.name == name) {
            Some(a) => self.theme.accent(&a.def.vendor, &a.def.model),
            None if name == "you" => self.theme.bright,
            None => self.theme.dim,
        }
    }

    pub fn handle(&mut self, msg: Msg) {
        match msg {
            Msg::Key(k) => self.key(k),
            Msg::Redraw => {}
            Msg::Deps {
                hash,
                added,
                missing,
            } => self.deps_checked(&hash, &added, &missing),
            Msg::Probe {
                agent,
                vendor,
                model,
                result,
            } => match result {
                Ok(()) => match config::set_model(&agent, &vendor, &model) {
                    Ok(()) => self.note(
                        "orda",
                        format!("{model} answered: {agent} now runs on {vendor} {model}"),
                    ),
                    Err(e) => self.note("orda", format!("could not switch {agent}: {e}")),
                },
                Err(why) => self.note(
                    "orda",
                    format!("{agent} stays as it is: {model} does not run on this account ({why})"),
                ),
            },
            Msg::Agent(i, id, ev) => {
                if self.agents.get(i).is_some_and(|a| a.run_id == id) {
                    self.agent_event(i, ev);
                }
            }
        }
    }

    /// Returns true when the screen needs a redraw.
    pub fn tick(&mut self) -> bool {
        // ticks come every 40 ms for smooth flights; the spinner still turns every 140 ms
        self.frame = (self.started.elapsed().as_millis() / 140) as usize;
        let now = Instant::now();
        let mut dirty = false;
        let landed: Vec<u32> = self
            .flights
            .iter()
            .filter(|f| f.progress() >= 1.0)
            .map(|f| f.msg)
            .collect();
        self.flights.retain(|f| f.progress() < 1.0);
        for id in landed {
            self.deliver(id);
        }
        let lit = self
            .agents
            .iter()
            .any(|a| a.flash.as_ref().is_some_and(|f| f.0.elapsed() < FLASH));
        dirty |= !self.flights.is_empty() || lit;
        if let Some(mut demo) = self.demo.take() {
            demo.step(self);
            self.demo = Some(demo);
            dirty = true;
        }
        if now - self.timers[0] >= SECOND {
            self.timers[0] = now;
            self.reload_config();
            self.check_resets();
            if !self.demo_mode {
                self.check_fresh();
                self.check_idle();
            }
            dirty = true; // the elapsed clock moves every second
        }
        if now - self.timers[1] >= 2 * SECOND {
            self.timers[1] = now;
            self.sample_tokens();
        }
        if now - self.timers[2] >= 5 * SECOND && !self.demo_mode {
            self.timers[2] = now;
            self.git = git::snapshot(&self.cwd);
            self.watch_commits();
        }
        if now - self.timers[3] >= 30 * SECOND {
            self.timers[3] = now;
            let live = std::mem::take(&mut self.limits);
            self.limits = limits::load();
            self.block_from_limit_watch();
            // keep windows that only came from live streams
            for s in live {
                if !self.limits.iter().any(|l| l.vendor == s.vendor) {
                    self.limits.push(s);
                }
            }
        }
        if self
            .toast
            .as_ref()
            .is_some_and(|t| t.2.elapsed() > 6 * SECOND)
        {
            self.toast = None;
            dirty = true;
        }
        dirty
            || self.toast.is_some()
            || !self.questions.is_empty()
            || self.agents.iter().any(|a| a.status.busy())
    }

    fn reload_config(&mut self) {
        let m = config::modified();
        if m == self.cfg_mtime {
            return;
        }
        self.cfg_mtime = m;
        let (cfg, err) = config::load();
        self.theme = Theme::build(&cfg.theme, &cfg.colors);
        // keep running agents; add new ones, update definitions of the rest
        for def in &cfg.agents {
            match self.agents.iter_mut().find(|a| a.def.name == def.name) {
                Some(a) if a.primary.is_some() => a.primary = Some(def.clone()),
                Some(a) => a.def = def.clone(),
                None => self.agents.push(Agent::new(def.clone())),
            }
        }
        self.agents
            .retain(|a| cfg.agents.iter().any(|d| d.name == a.def.name) || a.status.busy());
        self.cfg = cfg;
        match err {
            Some(e) => self.note("orda", e),
            None => self.note("orda", "settings reloaded"),
        }
    }

    fn sample_tokens(&mut self) {
        let mut totals: HashMap<String, u64> = HashMap::new();
        for v in ["claude", "codex", "gemini"] {
            totals.insert(v.into(), self.agents.iter().map(|a| a.tokens_on(v)).sum());
        }
        for (vendor, total) in totals {
            let prev = self
                .last_totals
                .insert(vendor.clone(), total)
                .unwrap_or(total);
            let h = self.spark.entry(vendor).or_default();
            h.push_back(total.saturating_sub(prev));
            while h.len() > 40 {
                h.pop_front();
            }
        }
    }

    fn key(&mut self, k: KeyEvent) {
        if k.modifiers.contains(KeyModifiers::CONTROL) && k.code == KeyCode::Char('c') {
            self.quit = true;
            return;
        }
        match self.focus {
            Focus::Input => match k.code {
                KeyCode::Esc => self.focus = Focus::None,
                KeyCode::Enter => {
                    let task = std::mem::take(&mut self.input).trim().to_string();
                    self.focus = Focus::None;
                    if !task.is_empty() {
                        self.submit(task);
                    }
                }
                KeyCode::Backspace => {
                    self.input.pop();
                }
                KeyCode::Char(c) => self.input.push(c),
                _ => {}
            },
            Focus::Questions => match k.code {
                KeyCode::Esc | KeyCode::Char('q') => self.focus = Focus::None,
                KeyCode::Char('j') | KeyCode::Down => {
                    self.q_sel = (self.q_sel + 1).min(self.questions.len().saturating_sub(1))
                }
                KeyCode::Char('k') | KeyCode::Up => self.q_sel = self.q_sel.saturating_sub(1),
                KeyCode::Char(c @ '1'..='9') => self.pick(c as usize - '1' as usize),
                KeyCode::Char('p') => self.answer(Scope::Project),
                KeyCode::Char('a') => self.answer(Scope::All),
                KeyCode::Char('o') => self.answer(Scope::Once),
                _ => {}
            },
            Focus::None => match k.code {
                KeyCode::Char('i') | KeyCode::Char('/') => self.focus = Focus::Input,
                KeyCode::Char('s') => self.scan(),
                KeyCode::Esc => self.zoom = None,
                KeyCode::Tab | KeyCode::BackTab => {
                    let mods = self.modules();
                    let cur = self.zoom.unwrap_or(self.tab);
                    let i = mods.iter().position(|m| *m == cur).unwrap_or(0);
                    let n = mods.len();
                    let next = mods[if k.code == KeyCode::Tab {
                        (i + 1) % n
                    } else {
                        (i + n - 1) % n
                    }];
                    self.show(next);
                }
                KeyCode::Char(c) => {
                    if let Some(m) = Module::from_key(c) {
                        if m == Module::Questions {
                            self.focus = Focus::Questions;
                            self.toast = None;
                            if self.compact || self.zoom.is_some() {
                                self.show(m);
                            }
                        } else if !self.compact && self.zoom == Some(m) {
                            self.zoom = None;
                        } else {
                            self.show(m);
                        }
                    }
                }
                _ => {}
            },
        }
    }

    fn show(&mut self, m: Module) {
        if self.compact {
            self.tab = m;
        } else {
            self.zoom = Some(m);
        }
    }

    fn pick(&mut self, option: usize) {
        let Some(q) = self.questions.get_mut(self.q_sel) else {
            return;
        };
        if option >= q.options.len() {
            return;
        }
        q.chosen = Some(option);
        // guard, scout and scanner questions have no scope step
        if q.guard_cmd.is_some() || q.change.is_some() || q.release.is_some() {
            self.answer(Scope::Once);
        }
    }

    fn answer(&mut self, scope: Scope) {
        let Some(q) = self.questions.get(self.q_sel) else {
            return;
        };
        let Some(choice) = q.chosen else { return };
        let q = self.questions.remove(self.q_sel);
        self.q_sel = self.q_sel.min(self.questions.len().saturating_sub(1));
        let answer = q.options[choice].clone();
        let where_ = match scope {
            Scope::All => "remembered for all projects",
            Scope::Project => "remembered for this project",
            Scope::Once => "just this once",
        };
        self.note("you", format!("{}: {answer} ({where_})", q.from));
        if let Some(cmd) = &q.guard_cmd {
            let allowed = choice == 0;
            if let Some(i) = self.agent_index(&q.from)
                && allowed
                && !self.demo_mode
            {
                let task = format!(
                    "The owner allowed you to run this command once: `{cmd}`. Run it now, then carry on with your task:\n\n{}",
                    self.agents[i].goal
                );
                self.queue_task(i, task);
            }
            if let Some(i) = self.agent_index(&q.from) {
                self.agents[i].status = Status::Running;
                self.agents[i].now = if allowed {
                    format!("running {cmd}")
                } else {
                    "skipped a denied command".into()
                };
            }
            if let Some(e) = self
                .guard_events
                .iter_mut()
                .find(|e| e.cmd == *cmd && e.verdict == Verdict::Ask)
            {
                e.why = if allowed {
                    "you allowed it once".into()
                } else {
                    "you said no".into()
                };
            }
            return;
        }
        if let Some(commit) = &q.release {
            if choice == 1 {
                self.holds.retain(|h| h.0 != *commit);
                self.note("you", format!("{commit}: a test value, released"));
            } else {
                self.note(
                    "you",
                    format!("{commit}: real, rotating it; the hold stays until aegis releases it"),
                );
            }
            return;
        }
        if let Some(c) = &q.change {
            let text = self.apply_change(c, choice == 0);
            if !self.demo_mode {
                save_answer(&self.cwd, &text, Scope::All);
            }
            self.remembered.insert(
                0,
                Remembered {
                    text,
                    scope: Scope::All,
                },
            );
            return;
        }
        let text = format!("{}: {answer}", short(&q.text, 40).trim_end_matches('?'));
        if scope != Scope::Once && !self.demo_mode {
            save_answer(&self.cwd, &text, scope);
        }
        self.remembered.insert(0, Remembered { text, scope });
        if self.questions.is_empty() {
            self.focus = Focus::None;
        }
    }

    fn submit(&mut self, task: String) {
        self.note("you", format!("task: {task}"));
        self.task = task.clone();
        self.task_started = Some(Instant::now());
        self.task_done = None;
        self.nudges = 0;
        if !self.demo_mode {
            match work::ensure_repo(std::path::Path::new(&self.cwd)) {
                Ok(true) => {
                    self.note(
                        "orda",
                        "this folder was not a git repo: made it one, with a first commit",
                    );
                    self.seen
                        .extend(git::recent_all(&self.cwd).into_iter().map(|c| c.0));
                }
                Ok(false) => {}
                Err(e) => {
                    self.note(
                        "orda",
                        format!("orda needs a git repo here and could not make one: {e}"),
                    );
                    return;
                }
            }
            self.git = git::snapshot(&self.cwd);
            // the periodic reviews start with the first real task, not when orda opens
            if !self.jobs_started {
                self.jobs_started = true;
                let stale = |job: &str, every: i64| {
                    stats::last_run(job).is_none_or(|t| now_unix() - t > every)
                };
                if stale("scout", SCOUT_EVERY) {
                    self.scan();
                }
                if stale("aegis-audit", AUDIT_EVERY) && git::recent_all(&self.cwd).len() > 3 {
                    self.audit();
                }
            }
        }
        if self.demo_mode {
            self.note(
                "boss",
                "demo mode: start orda without --demo to run real agents",
            );
            return;
        }
        let Some(i) = self.agents.iter().position(|a| a.def.role == "boss") else {
            self.note("orda", "no agent has role = \"boss\" in the config");
            return;
        };
        if self.agents[i].status.busy() {
            self.note("orda", "the boss is still working on the last task");
            return;
        }
        // anchor writes the acceptance criteria first, when it is on the team
        match self.agents.iter().position(|a| a.def.role == "anchor") {
            Some(k) if !self.agents[k].status.busy() => {
                self.flow = Some((Stage::Criteria, task.clone()));
                self.note("anchor", "writing the acceptance criteria");
                self.start(k, &format!("Write the acceptance criteria for this task from the owner, before any work starts:\n\n{task}"));
            }
            _ => self.start(i, &task),
        }
    }

    /// Run one agent on a task, with its role's instructions.
    fn start(&mut self, i: usize, task: &str) {
        let mut task = task.to_string();
        if self.agents[i].def.role == "boss" && !self.holds.is_empty() {
            task += "\n\n## Commits on hold: do not merge these\n\n";
            for (c, why, who) in &self.holds {
                task += &format!("- {c}, held by {who}: {why}\n");
            }
        }
        if !self.agents[i].mailbox.is_empty() {
            task += "\n\n## Cards for you since your last run (no answer needed)\n\n";
            for m in self.agents[i].mailbox.drain(..) {
                task += &format!("- {m}\n");
            }
        }
        // the boss is copied on every message between its team
        if self.agents[i].def.role == "boss" && !self.digest.is_empty() {
            task += "\n\n## Messages between your team since your last run\n\n";
            for d in self.digest.drain(..) {
                task += &format!("- {d}\n");
            }
        }
        let task = task.as_str();
        // agents that write code or check commits work in their own worktree
        let dir = if work::in_worktree(&self.agents[i].def.role) && !self.demo_mode {
            let main = self
                .git
                .as_ref()
                .map(|g| g.branch.clone())
                .filter(|b| !b.is_empty())
                .unwrap_or("main".into());
            let job = match task
                .strip_prefix("New commit ")
                .and_then(|t| t.split_whitespace().next())
            {
                Some(commit) => work::Job::Check { commit },
                None => work::Job::Task { main: &main },
            };
            let name = self.agents[i].def.name.clone();
            match work::prepare(std::path::Path::new(&self.cwd), &name, job) {
                Ok(p) => p,
                Err(e) => {
                    self.note(&name, format!("could not prepare its worktree: {e}"));
                    self.agents[i].status = Status::Failed;
                    return;
                }
            }
        } else {
            std::path::PathBuf::from(&self.cwd)
        };
        let a = &mut self.agents[i];
        a.dir = dir;
        a.goal = task.to_string();
        a.journal.clear();
        a.run = Some((Instant::now(), a.tokens_in, a.tokens_out, a.tools));
        a.last_report.clear();
        self.launch(i, task.to_string());
    }

    /// Run agent `i` on `prompt` with the best model that still has limits left.
    fn launch(&mut self, i: usize, prompt: String) {
        if !self.route(i) {
            return;
        }
        let team: Vec<AgentDef> = self.agents.iter().map(|a| a.def.clone()).collect();
        let decided: Vec<String> = self
            .remembered
            .iter()
            .filter(|r| r.scope != Scope::Once)
            .map(|r| r.text.clone())
            .collect();
        let a = &mut self.agents[i];
        a.status = Status::Running;
        a.now = "starting".into();
        if !a.goal.is_empty() {
            a.task = task_label(&a.goal);
        }
        a.waiting = false;
        a.run_id += 1;
        if let Some(h) = a.handle.take() {
            h.abort();
        }
        if self.demo_mode {
            return;
        }
        let (d, id) = (a.def.clone(), a.run_id);
        let read = |f: &str| {
            std::fs::read_to_string(std::path::Path::new(&self.cwd).join(f)).unwrap_or_default()
        };
        let lessons = roles::project_notes(&read("MAP.md"), &read("LESSONS.md"));
        let saving = savings::for_role(&self.savings, &d.role);
        let system = roles::instructions(&d, &team, &decided, &lessons, &saving);
        let dir = a.dir.clone();
        a.handle = vendors::spawn(&d, &system, &prompt, &dir, self.tx.clone(), move |e| {
            Msg::Agent(i, id, e)
        });
    }

    /// A vendor out of limits, or one model the vendor refuses (`vendor:model`).
    fn is_blocked(&self, key: &str) -> bool {
        self.blocked.get(key).is_some_and(|t| *t > now_unix())
    }

    fn model_blocked(&self, f: &Fallback) -> bool {
        self.is_blocked(&f.vendor) || self.is_blocked(&format!("{}:{}", f.vendor, f.model))
    }

    /// Put agent `i` on its own model, or on the first fallback whose limit is not used up.
    /// Returns false when every candidate is used up and the agent has to wait.
    fn route(&mut self, i: usize) -> bool {
        let own = self.agents[i]
            .primary
            .clone()
            .unwrap_or_else(|| self.agents[i].def.clone());
        let mut pick = Fallback {
            vendor: own.vendor.clone(),
            model: own.model.clone(),
            effort: own.effort.clone(),
        };
        let mut tried = vec![];
        while self.model_blocked(&pick) {
            tried.push(pick.vendor.clone());
            match self.cfg.fallback.get(&pick.vendor) {
                Some(next) if !tried.contains(&next.vendor) => pick = next.clone(),
                _ => {
                    let until = tried
                        .iter()
                        .filter_map(|v| self.blocked.get(v))
                        .min()
                        .copied()
                        .unwrap_or(0);
                    let a = &mut self.agents[i];
                    a.waiting = true;
                    a.status = Status::Queued;
                    a.now = format!("out of limits, resumes at {}", hhmm(until));
                    let name = a.def.name.clone();
                    self.note(
                        &name,
                        format!(
                            "every model it can use is out of limits, waits until {}",
                            hhmm(until)
                        ),
                    );
                    return false;
                }
            }
        }
        let a = &mut self.agents[i];
        let name = a.def.name.clone();
        let from = format!("{} ({})", a.def.model, a.def.vendor);
        if pick.vendor == own.vendor && pick.model == own.model {
            if a.primary.is_some() {
                a.carry();
                a.primary = None;
                a.def = own;
                let old = from.split(' ').next().unwrap_or("").to_string();
                let to = a.def.model.clone();
                self.note(&name, format!("back on {to}, switched from {old}"));
            }
        } else if a.def.vendor != pick.vendor || a.def.model != pick.model {
            a.primary.get_or_insert_with(|| own.clone());
            a.carry();
            a.def.vendor = pick.vendor.clone();
            a.def.model = pick.model.clone();
            a.def.effort = pick.effort.clone();
            let old = from.split(' ').next().unwrap_or("").to_string();
            self.note(
                &name,
                format!("switched {old} -> {} ({})", pick.model, pick.vendor),
            );
        }
        true
    }

    /// `orda --demo` only: play a limit hit without a real run.
    pub fn demo_limit_hit(&mut self, i: usize, resets_at: i64) {
        self.agents[i].journal = vec![
            "edit src/agents/codex.rs".into(),
            "shell cargo test".into(),
            "  failed: 2 tests".into(),
        ];
        self.limit_hit(i, Some(resets_at), "You've hit your usage limit (demo)");
        self.agents[i].now = "picking up where gpt-6-sol stopped".into();
    }

    /// The vendor will not run this model for this account: take it off the list for the
    /// day and hand the same task to the fallback, like a used-up limit.
    fn model_unavailable(&mut self, i: usize, msg: &str) {
        let (vendor, model, name) = {
            let d = &self.agents[i].def;
            (d.vendor.clone(), d.model.clone(), d.name.clone())
        };
        self.blocked
            .insert(format!("{vendor}:{model}"), now_unix() + 12 * 3600);
        self.note(
            &name,
            format!("{model} cannot run on this account: {}", short(msg, 90)),
        );
        let a = &mut self.agents[i];
        if let Some(h) = a.handle.take() {
            h.abort();
        }
        let prompt = handoff(
            &a.goal,
            &a.journal,
            &format!("{model} ({vendor})"),
            "the vendor refused the model",
        );
        self.launch(i, prompt);
    }

    /// Tell the boss an agent's run failed, so its work does not silently stop.
    fn report_failure(&mut self, i: usize, err: &str) {
        let a = &self.agents[i];
        if self.demo_mode || a.def.role == "boss" || a.goal.is_empty() {
            return;
        }
        let card = roles::Card {
            to: "boss".into(),
            re: None,
            kind: "failure".into(),
            fields: vec![
                (
                    "title".into(),
                    format!("{} failed: {}", a.def.name, short(err, 120)),
                ),
                ("task".into(), short(&a.goal, 300)),
                (
                    "ask".into(),
                    "hand this work to someone else, or try again".into(),
                ),
            ],
        };
        let from = a.def.name.clone();
        if self.agent_index("boss").is_some() {
            self.send(&from, card);
        }
    }

    fn finish_task(&mut self) {
        if self.task_done.is_none() {
            self.task_done = Some(Instant::now());
            let took = self
                .task_started
                .map(|t| t.elapsed().as_secs())
                .unwrap_or(0);
            self.note(
                "orda",
                format!("the task is finished, in {}m{:02}s", took / 60, took % 60),
            );
            if self.cfg.notify.desktop && !self.demo_mode {
                let _ = std::process::Command::new("notify-send")
                    .args(["-a", "orda", "orda: task finished", &self.task])
                    .spawn();
            }
        }
    }

    /// What the owner's task is doing right now, for the header.
    pub fn task_state(&self) -> (String, Status) {
        if self.task.is_empty() {
            return ("no task yet".into(), Status::Idle);
        }
        if self.task_done.is_some() {
            return ("finished".into(), Status::Done);
        }
        let busy = self.agents.iter().filter(|a| a.status.busy()).count();
        if busy > 0 {
            return (
                format!("working, {busy} agent{}", if busy == 1 { "" } else { "s" }),
                Status::Running,
            );
        }
        if !self.questions.is_empty() {
            return ("waiting for you, press q".into(), Status::Asking);
        }
        if self.agents.iter().any(|a| a.waiting) {
            return ("waiting for a limit to reset".into(), Status::Queued);
        }
        if self.nudges >= MAX_NUDGES {
            return (
                "stalled: the boss could not move it on".into(),
                Status::Failed,
            );
        }
        ("idle, waking the boss".into(), Status::Queued)
    }

    /// Nothing running, nothing queued, the task not finished: wake the boss, a few times at most.
    fn check_idle(&mut self) {
        if self.task.is_empty() || self.task_done.is_some() || self.nudges >= MAX_NUDGES {
            return;
        }
        let quiet =
            self.agents.iter().all(|a| {
                !a.status.busy() && a.queue.is_empty() && a.inbox.is_empty() && !a.waiting
            }) && self.flights.is_empty()
                && self.questions.is_empty();
        if !quiet {
            self.quiet_since = None;
            return;
        }
        // commits are noticed every 5 seconds and agents start right after each other:
        // only a stretch of real quiet counts
        let since = *self.quiet_since.get_or_insert_with(Instant::now);
        if since.elapsed() < QUIET {
            return;
        }
        self.quiet_since = None;
        let Some(b) = self.agents.iter().position(|a| a.def.role == "boss") else {
            return;
        };
        self.nudges += 1;
        self.note(
            "orda",
            format!(
                "nothing is running and the task is not finished: woke the boss ({}/{MAX_NUDGES})",
                self.nudges
            ),
        );
        let pending: Vec<String> = self
            .checks
            .iter()
            .map(|(h, (b, p))| format!("{h} on {b}: still checking {}", p.join(", ")))
            .collect();
        let holds: Vec<String> = self
            .holds
            .iter()
            .map(|(h, why, who)| format!("{h} held by {who}: {why}"))
            .collect();
        let task = format!(
            "Nothing is running and the owner's task is not finished.\n\nThe task:\n{}\n\nCommits still being checked: {}\nHeld commits: {}\n\nLook at the branches (`git log --oneline --all`), decide what is next, and act: hand out work with task cards, merge what passed, or write FINISHED: if everything is merged and done.",
            self.task,
            if pending.is_empty() {
                "none".into()
            } else {
                pending.join("; ")
            },
            if holds.is_empty() {
                "none".into()
            } else {
                holds.join("; ")
            },
        );
        self.start(b, &task);
    }

    /// A vendor refused for limits: block it, then hand the same task to the next model.
    fn limit_hit(&mut self, i: usize, resets_at: Option<i64>, message: &str) {
        let now = now_unix();
        let a = &mut self.agents[i];
        let vendor = a.def.vendor.clone();
        let name = a.def.name.clone();
        let old = format!("{} ({})", a.def.model, a.def.vendor);
        let until = resets_at.filter(|t| *t > now).unwrap_or(now + 30 * 60);
        self.blocked.insert(vendor.clone(), until);
        self.note(
            &name,
            format!(
                "{vendor} limit hit, back at {}: {}",
                hhmm(until),
                short(message, 60)
            ),
        );
        let a = &mut self.agents[i];
        if let Some(h) = a.handle.take() {
            h.abort();
        }
        let prompt = handoff(&a.goal, &a.journal, &old, "its plan limit ran out");
        self.launch(i, prompt);
    }

    /// Unblock vendors whose limit reset; move agents back and wake the waiting ones.
    fn check_resets(&mut self) {
        let now = now_unix();
        let reset: Vec<String> = self
            .blocked
            .iter()
            .filter(|(_, t)| **t <= now)
            .map(|(v, _)| v.clone())
            .collect();
        for v in reset {
            self.blocked.remove(&v);
            self.note("orda", format!("{v} limit reset"));
            for i in 0..self.agents.len() {
                let a = &mut self.agents[i];
                if a.waiting {
                    let prompt = handoff(
                        &a.goal,
                        &a.journal,
                        "the previous model",
                        "its plan limit ran out",
                    );
                    self.launch(i, prompt);
                } else if a.primary.as_ref().is_some_and(|p| p.vendor == v) {
                    if a.status.busy() {
                        a.return_pending = true;
                        let (name, model) = (
                            a.def.name.clone(),
                            a.primary.as_ref().unwrap().model.clone(),
                        );
                        self.note(&name, format!("goes back to {model} when this run ends"));
                    } else {
                        self.route(i);
                    }
                }
            }
        }
    }

    /// limit-watch already knows a window is used up: block that vendor before anyone hits it.
    fn block_from_limit_watch(&mut self) {
        let now = now_unix();
        for s in self.limits.iter().filter(|s| s.error.is_none()) {
            for w in s.windows.iter().filter(|w| {
                (w.label == "5h" || w.label == "7d") && w.used_pct >= 100.0 && w.resets_at > now
            }) {
                let t = self.blocked.entry(s.vendor.clone()).or_insert(w.resets_at);
                *t = (*t).max(w.resets_at);
            }
        }
    }

    /// An agent sent a card to another agent: number it, thread it, copy the boss,
    /// and start it flying. It is delivered when it lands.
    pub fn send(&mut self, from: &str, card: roles::Card) -> Option<u32> {
        if self.agent_index(&card.to).is_none() {
            self.note(
                from,
                format!("sent a message to \"{}\", who is not on the team", card.to),
            );
            return None;
        }
        let id = self.messages.last().map_or(1, |m| m.id + 1);
        let thread = card
            .re
            .and_then(|r| self.messages.iter().find(|m| m.id == r))
            .map_or(id, |m| m.thread);
        let missing = card.missing();
        if !missing.is_empty() {
            self.note(from, format!("#{id} is missing {}", missing.join(", ")));
        }
        let title = card.get("title").to_string();
        let (kind, to) = (card.kind.clone(), card.to.clone());
        self.note(from, format!("{kind} #{id} -> {to}: {title}"));
        self.digest
            .push(format!("#{id} {from} -> {to} ({kind}): {title}"));
        self.messages.push(mail::Message {
            id,
            thread,
            from: from.into(),
            to: to.clone(),
            card,
        });
        self.flights.push(mail::Flight {
            from: from.into(),
            to: to.clone(),
            label: format!("{kind} #{id}"),
            kind: kind.clone(),
            start: Instant::now(),
            msg: id,
        });
        // a claim about a commit goes to the referee, once per commit
        let claimed = self
            .messages
            .last()
            .map(|m| {
                m.card
                    .get("commit")
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .to_string()
            })
            .unwrap_or_default();
        let referee = self.agents.iter().position(|a| a.def.role == "referee");
        if let Some(r) = referee
            && matches!(kind.as_str(), "fixed" | "done")
            && from != "referee"
            && self.agents[r].def.name != to
            && !claimed.is_empty()
            && !self.demo_mode
        {
            let name = self.agents[r].def.name.clone();
            let already = !self.verified.insert(claimed.clone())
                || self.checks.iter().any(|(h, (_, p))| {
                    (h.starts_with(&claimed) || claimed.starts_with(h.as_str()))
                        && p.contains(&name)
                });
            if !already {
                let msgs: Vec<&mail::Message> = self
                    .messages
                    .iter()
                    .filter(|m| m.thread == thread)
                    .collect();
                let task = format!(
                    "New commit {claimed} (claimed in #{id}). Verify this claim before anyone relies on it.\n\n{}",
                    mail::prompt(&msgs, "referee")
                );
                self.queue_task(r, task);
            }
        }
        // the same bug a third time: the pair is stuck, the boss and the advisor take a look
        let rounds = self
            .messages
            .iter()
            .filter(|m| m.thread == thread && m.card.kind == "bug")
            .count();
        if kind == "bug"
            && rounds >= mail::ROUNDS
            && self.agent_index("boss").is_some()
            && from != "boss"
        {
            self.note(
                "orda",
                format!("#{thread} came back {rounds} times, the boss and the advisor take over"),
            );
            let card = roles::Card {
                to: "boss".into(),
                re: Some(id),
                kind: "escalation".into(),
                fields: vec![
                    (
                        "title".into(),
                        format!("thread #{thread} failed {rounds} rounds, needs another approach"),
                    ),
                    (
                        "ask".into(),
                        "consult the advisor, then decide: new approach, other builder, or drop it"
                            .into(),
                    ),
                ],
            };
            self.send(from, card);
        }
        Some(id)
    }

    /// Hand a landed message to its receiver: now if it is free, after its run if busy.
    fn deliver(&mut self, id: u32) {
        let Some(m) = self.messages.iter().find(|m| m.id == id) else {
            return;
        };
        let Some(i) = self.agent_index(&m.to) else {
            return;
        };
        let (kind, thread, from) = (m.card.kind.clone(), m.thread, m.from.clone());
        let rounds = self
            .messages
            .iter()
            .filter(|x| x.thread == thread && x.card.kind == "bug")
            .count();
        let a = &mut self.agents[i];
        a.flash = Some((Instant::now(), kind.clone()));
        a.say(format!("got {kind} #{id} from {from}"));
        a.now = match kind.as_str() {
            "bug" => format!("fixing #{thread}, round {rounds}/{}", mail::ROUNDS),
            "fixed" => format!("retesting #{thread}"),
            "review" => format!("working through review #{id}"),
            "done" => format!("#{thread} verified"),
            "escalation" => format!("rethinking #{thread} with the advisor"),
            _ => format!("reading #{id} from {from}"),
        };
        if self.demo_mode {
            return;
        }
        if !mail::wakes(&kind) {
            let title = self
                .messages
                .iter()
                .find(|m| m.id == id)
                .map(|m| m.card.get("title").to_string())
                .unwrap_or_default();
            self.agents[i]
                .mailbox
                .push(format!("#{id} from {from} ({kind}): {title}"));
            return;
        }
        let a = &mut self.agents[i];
        if a.status.busy() {
            a.inbox.push_back(id);
        } else {
            self.deliver_now(i, id);
        }
    }

    fn deliver_now(&mut self, i: usize, id: u32) {
        let Some(thread) = self.messages.iter().find(|m| m.id == id).map(|m| m.thread) else {
            return;
        };
        let msgs: Vec<&mail::Message> = self
            .messages
            .iter()
            .filter(|m| m.thread == thread && m.id <= id)
            .collect();
        let prompt = mail::prompt(&msgs, &self.agents[i].def.name);
        self.start(i, &prompt);
    }

    /// The paired review: the bursar goes first, and the scout starts with its report.
    pub fn scan(&mut self) {
        match self.agents.iter().position(|a| a.def.role == "bursar") {
            Some(i) if !self.demo_mode && !self.agents[i].status.busy() => self.bursar(i),
            Some(_) if self.demo_mode => self.note(
                "bursar",
                "demo mode: start orda without --demo to run a real review",
            ),
            _ => self.scout(None),
        }
    }

    fn limits_text(&self) -> String {
        let mut limits = String::new();
        for s in &self.limits {
            for w in &s.windows {
                limits += &format!("- {} {}: {:.0}% used\n", s.vendor, w.label, w.used_pct);
            }
        }
        if limits.is_empty() {
            "unknown\n".into()
        } else {
            limits
        }
    }

    fn bursar(&mut self, i: usize) {
        let runs = stats::runs();
        let since = stats::last_run("bursar")
            .map(|t| {
                format!(
                    "Your last review was {} days ago.",
                    (now_unix() - t) / 86400
                )
            })
            .unwrap_or("This is your first review.".into());
        let trend = stats::trend(&runs, now_unix())
            .map(|t| format!("Tokens per run this week against last week: {t}."))
            .unwrap_or_default();
        let task = format!(
            "Run your review. Today is {}. {since} {trend}\n\n## Spending per role and model\n\n{}\n## Your rules and their measured effect\n\n{}\n## Plan limits right now\n\n{}",
            today(),
            stats::summary(),
            savings::report(&self.savings, &runs),
            self.limits_text()
        );
        self.note("bursar", "reviewing what the team spends");
        self.start(i, &task);
    }

    /// Start the scout's scan of new models, with orda's own record attached.
    fn scout(&mut self, bursar_report: Option<String>) {
        let Some(i) = self.agents.iter().position(|a| a.def.role == "scout") else {
            self.note("orda", "no agent has role = \"scout\" in the config");
            return;
        };
        if self.demo_mode {
            self.note(
                "scout",
                "demo mode: start orda without --demo to run a real scan",
            );
            return;
        }
        if self.agents[i].status.busy() {
            return;
        }
        let since = stats::last_run("scout")
            .map(|t| format!("Your last scan was {} days ago.", (now_unix() - t) / 86400))
            .unwrap_or("This is your first scan.".into());
        let mut task = format!(
            "Run your scan. Today is {}. {since}\n\n## orda's own record\n\n{}\n## Plan limits right now\n\n{}",
            today(),
            stats::summary(),
            self.limits_text()
        );
        if let Some(r) = bursar_report.filter(|r| !r.trim().is_empty()) {
            task += &format!("\n## The bursar's report from just now\n\n{r}");
        }
        self.note("scout", "scanning for new models and benchmarks");
        self.start(i, &task);
    }

    /// aegis's weekly look at the whole project.
    fn audit(&mut self) {
        let Some(i) = self.agents.iter().position(|a| a.def.role == "aegis") else {
            return;
        };
        if self.demo_mode || self.agents[i].status.busy() {
            return;
        }
        self.note("aegis", "weekly security audit of the whole project");
        self.start(i, &format!("{AUDIT_TASK} Today is {}.", today()));
    }

    /// Bookkeeping after a finished run: roll back savings rules that hurt, mark
    /// periodic jobs done, and hand the bursar's report to the scout.
    fn after_run(&mut self, i: usize, def: &AgentDef, ok: bool) {
        let now = now_unix();
        let runs = stats::runs();
        let gone = savings::judge(&mut self.savings, &runs);
        for (id, why) in &gone {
            self.note("bursar", format!("rolled back {id}: {why}"));
        }
        if !gone.is_empty() {
            savings::store(&self.savings);
        }
        self.burn = stats::trend(&runs, now);
        if !ok {
            return;
        }
        match def.role.as_str() {
            "scout" => stats::mark_run("scout", now),
            "aegis" if self.agents[i].goal.starts_with(AUDIT_TASK) => {
                stats::mark_run("aegis-audit", now)
            }
            "bursar" => {
                stats::mark_run("bursar", now);
                let report = self.agents[i].last_report.clone();
                self.scout(Some(report));
            }
            "anchor" => match self.flow.clone() {
                Some((Stage::Criteria, task)) => {
                    let criteria = self.agents[i].last_report.clone();
                    if let Some(b) = self.agents.iter().position(|a| a.def.role == "boss") {
                        self.flow = Some((Stage::Building, task.clone()));
                        self.start(b, &format!("{task}\n\n## Acceptance criteria from anchor (also in SPEC.md)\n\n{criteria}"));
                    }
                }
                Some((Stage::Verifying, task)) => {
                    // a bug card from anchor means criteria are missing: the boss is on it
                    let missing = self.messages.iter().any(|m| {
                        m.id >= self.verify_from
                            && m.from == self.agents[i].def.name
                            && m.card.kind == "bug"
                    });
                    if missing {
                        self.flow = Some((Stage::Building, task));
                    } else {
                        self.flow = None;
                        self.finish_task();
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }

    /// Give an agent a task now, or after its current run.
    fn queue_task(&mut self, i: usize, task: String) {
        let a = &self.agents[i];
        if a.status.busy() || a.waiting {
            self.agents[i].queue.push_back(task);
        } else {
            self.start(i, &task);
        }
    }

    /// After a run: a waiting message first, then a queued task.
    fn next_queued(&mut self, i: usize) {
        if self.agents[i].status.busy() {
            return;
        }
        if let Some(t) = self.agents[i].queue.pop_front() {
            self.start(i, &t);
        }
    }

    /// New commits since the last look: through the secret scanner, then to the always-on agents.
    fn watch_commits(&mut self) {
        if self.git.is_none() {
            return;
        }
        // every branch, oldest first: builders commit on their own wt/ branches
        let fresh: Vec<(String, String)> = git::recent_all(&self.cwd)
            .into_iter()
            .rev()
            .filter(|c| !self.seen.contains(&c.0))
            .collect();
        for (hash, subject) in fresh {
            self.seen.insert(hash.clone());
            self.nudges = 0;
            let diff = git::show(&self.cwd, &hash);
            let files = git::files(&self.cwd, &hash);
            let found = secrets::scan_diff(&diff);
            if !found.is_empty() {
                self.scanner_hit(&hash, &found);
            }
            let bent = integrity::scan_diff(&diff);
            if !bent.is_empty() {
                self.integrity_hit(&hash, &bent);
            }
            let manifest = files.iter().any(|f| deps::is_manifest(f));
            if manifest && !self.demo_mode {
                // registry lookups take seconds; they report back as Msg::Deps
                let (dir, h, tx, fs) = (
                    self.cwd.clone(),
                    hash.clone(),
                    self.tx.clone(),
                    files.clone(),
                );
                tokio::task::spawn_blocking(move || {
                    let added = deps::added(&dir, &h, &fs);
                    let missing = added
                        .iter()
                        .filter(|(e, n)| deps::exists(e, n) == Some(false))
                        .cloned()
                        .collect();
                    let _ = tx.send(Msg::Deps {
                        hash: h,
                        added,
                        missing,
                    });
                });
            }
            // empty commits and notes or docs need no checking crew
            let docs_only = files.iter().all(|f| f.ends_with(".md"));
            if !self.cfg.watch.commits || docs_only {
                continue;
            }
            let owners = work::owners(std::path::Path::new(&self.cwd), &hash);
            let branch = owners
                .first()
                .map(|o| format!("wt/{o}"))
                .or_else(|| self.git.as_ref().map(|g| g.branch.clone()))
                .unwrap_or_default();
            let mut pending = vec![];
            for i in 0..self.agents.len() {
                let d = &self.agents[i].def;
                // customs only cares about what comes in from outside; nobody checks their own commit
                if d.always_on && (d.role != "customs" || manifest) && !owners.contains(&d.name) {
                    pending.push(d.name.clone());
                    let task = format!(
                        "New commit {hash} on {branch}: \"{subject}\". Check it as your role describes. Your folder is set to this commit."
                    );
                    self.queue_task(i, task);
                }
            }
            if !pending.is_empty() && branch.starts_with("wt/") {
                self.checks.insert(hash.clone(), (branch, pending));
            }
        }
    }

    /// An agent finished checking a commit: when every check is in and nothing holds it, wake the boss.
    fn check_done(&mut self, who: &str, goal: &str) {
        let Some(hash) = goal
            .strip_prefix("New commit ")
            .and_then(|t| t.split_whitespace().next())
        else {
            return;
        };
        let hash = hash.to_string();
        if let Some((_, pending)) = self.checks.get_mut(&hash) {
            pending.retain(|n| n != who);
        }
        self.maybe_merge(&hash);
    }

    fn maybe_merge(&mut self, hash: &str) {
        let Some((branch, pending)) = self.checks.get(hash) else {
            return;
        };
        if !pending.is_empty()
            || self
                .holds
                .iter()
                .any(|h| h.0.starts_with(hash) || hash.starts_with(h.0.as_str()))
        {
            return;
        }
        let branch = branch.clone();
        self.checks.remove(hash);
        let main = self
            .git
            .as_ref()
            .map(|g| g.branch.clone())
            .unwrap_or("main".into());
        self.note(
            "orda",
            format!("{hash} passed every check, the boss can merge {branch}"),
        );
        if let Some(b) = self.agents.iter().position(|a| a.def.role == "boss") {
            let task = format!(
                "Commit {hash} on {branch} passed every check and nothing holds it. If the work on that branch is finished, merge it: `git merge --no-ff {branch}` while on {main}. Then hand out what is next, or report the task finished."
            );
            self.queue_task(b, task);
        }
    }

    /// The free test-integrity check saw signs of a bent test: the referee judges.
    pub fn integrity_hit(&mut self, hash: &str, found: &[secrets::Finding]) {
        let list: Vec<String> = found
            .iter()
            .take(4)
            .map(|f| {
                if f.line == 0 {
                    format!("{} in {}", f.what, f.file)
                } else {
                    format!("{} at {}:{}", f.what, f.file, f.line)
                }
            })
            .collect();
        self.note("integrity", format!("{hash}: {}", list.join(", ")));
        if self.agent_index("referee").is_some() {
            let card = roles::Card {
                to: "referee".into(),
                re: None,
                kind: "note".into(),
                fields: vec![
                    (
                        "title".into(),
                        format!("possible bent test in {hash}: {}", list.join(", ")),
                    ),
                    (
                        "ask".into(),
                        "cleanup or cheat? hold it if the test no longer means what it did".into(),
                    ),
                ],
            };
            self.send("integrity", card);
        }
    }

    /// The registry answered for a commit's new packages: a package that does not exist is held at once.
    pub fn deps_checked(
        &mut self,
        hash: &str,
        added: &[(&'static str, String)],
        missing: &[(&'static str, String)],
    ) {
        if added.is_empty() {
            return;
        }
        let names = |l: &[(&'static str, String)]| {
            l.iter()
                .map(|(e, n)| format!("{n} ({e})"))
                .collect::<Vec<_>>()
                .join(", ")
        };
        if missing.is_empty() {
            self.note(
                "deps",
                format!("{hash}: new packages exist: {}", names(added)),
            );
            return;
        }
        let why = format!("not in the registry: {}", names(missing));
        self.holds.retain(|h| h.0 != hash);
        self.holds
            .push((hash.into(), why.clone(), "customs".into()));
        self.note("deps", format!("hold on {hash}: {why}"));
        if self.agent_index("customs").is_some() {
            let card = roles::Card {
                to: "customs".into(),
                re: None,
                kind: "note".into(),
                fields: vec![
                    ("title".into(), format!("{hash} adds packages that do not exist: {}", names(missing))),
                    ("ask".into(), "invented name or a typo of a real package? tell the builder; RELEASE when fixed".into()),
                ],
            };
            self.send("deps", card);
        }
    }

    /// Restart a run that went on too long, with a handoff of everything it did.
    pub fn refresh(&mut self, i: usize, why: &str) {
        let a = &mut self.agents[i];
        let name = a.def.name.clone();
        let prompt = handoff(&a.goal, &a.journal, "Your previous run", why);
        let size = prompt.len() / 4;
        self.note(
            &name,
            format!("fresh context ({why}), handoff about {} tokens", size),
        );
        self.launch(i, prompt);
    }

    /// The limits in [fresh]: any running agent past one gets a fresh start.
    fn check_fresh(&mut self) {
        let f = self.cfg.fresh.clone();
        for i in 0..self.agents.len() {
            let a = &self.agents[i];
            let Some((t0, _, _, c0)) = a.run else {
                continue;
            };
            if !a.status.busy() {
                continue;
            }
            let why = if t0.elapsed().as_secs() > f.minutes * 60 {
                format!("over {} minutes", f.minutes)
            } else if a.tools - c0 > f.tool_calls {
                format!("over {} tool calls", f.tool_calls)
            } else if a.context > f.context {
                format!("context over {}k", f.context / 1000)
            } else {
                continue;
            };
            // the counters start again with the new run
            self.agents[i].run = Some((
                Instant::now(),
                self.agents[i].tokens_in,
                self.agents[i].tokens_out,
                self.agents[i].tools,
            ));
            self.agents[i].context = 0;
            self.refresh(i, &why);
        }
    }

    /// The scanner found possible secrets or personal data in a commit: hold it, ask the owner, tell aegis.
    pub fn scanner_hit(&mut self, hash: &str, found: &[secrets::Finding]) {
        let list: Vec<String> = found
            .iter()
            .take(3)
            .map(|f| {
                if f.line == 0 {
                    format!("{} ({})", f.what, f.file)
                } else {
                    format!("{} in {}:{}", f.what, f.file, f.line)
                }
            })
            .collect();
        let more = if found.len() > 3 {
            format!(" and {} more", found.len() - 3)
        } else {
            String::new()
        };
        let summary = format!("{}{more}", list.join(", "));
        self.holds.retain(|h| h.0 != hash);
        self.holds
            .push((hash.into(), summary.clone(), "scanner".into()));
        self.note("scanner", format!("hold on {hash}: {summary}"));
        self.ask(Question {
            from: "scanner".into(),
            task: "security".into(),
            text: format!("Possible secret or personal data in {hash}: {summary}. If it is real, rotate it: deleting the line does not un-leak it."),
            options: vec!["it is real, I will rotate it".into(), "it is a test value, release it".into()],
            chosen: None,
            guard_cmd: None,
            change: None,
            release: Some(hash.into()),
            asked: Instant::now(),
        });
        if self.agent_index("aegis").is_some() {
            let card = roles::Card {
                to: "aegis".into(),
                re: None,
                kind: "note".into(),
                fields: vec![
                    ("title".into(), format!("scanner hit in {hash}: {summary}")),
                    (
                        "ask".into(),
                        "confirm or clear it; the commit stays held until you RELEASE it".into(),
                    ),
                ],
            };
            self.send("scanner", card);
        }
    }

    /// Approve or decline a scout recommendation. Returns what to remember.
    fn apply_change(&mut self, c: &Change, approve: bool) -> String {
        let old = self
            .agents
            .iter()
            .find(|a| a.def.name == c.agent)
            .map(|a| a.def.model.clone())
            .unwrap_or_default();
        if !approve {
            return format!("keep {} on {old}, not {} {}", c.agent, c.vendor, c.model);
        }
        if self.demo_mode {
            if let Some(a) = self.agents.iter_mut().find(|a| a.def.name == c.agent) {
                a.def.vendor = c.vendor.clone();
                a.def.model = c.model.clone();
            }
        } else {
            // a recommendation is only as good as the model actually running here: try it first
            let (agent, vendor, model, tx) = (
                c.agent.clone(),
                c.vendor.clone(),
                c.model.clone(),
                self.tx.clone(),
            );
            self.note(
                "orda",
                format!("trying {model} once before switching {agent}"),
            );
            tokio::task::spawn_blocking(move || {
                let result = vendors::probe(&vendor, &model);
                let _ = tx.send(Msg::Probe {
                    agent,
                    vendor,
                    model,
                    result,
                });
            });
            return format!("{} to {} {}, if it runs here", c.agent, c.vendor, c.model);
        }
        self.note(
            "orda",
            format!("{} now runs on {} {}", c.agent, c.vendor, c.model),
        );
        format!("{} moved from {old} to {} {}", c.agent, c.vendor, c.model)
    }

    /// What an agent asked for with an ASK:, REPORT: or LEARNED: line.
    fn protocol(&mut self, who: &str, task: &str, line: roles::Line) {
        match line {
            roles::Line::Ask { text, options } => self.ask(Question {
                from: who.into(),
                task: if task.is_empty() {
                    "task".into()
                } else {
                    task.into()
                },
                text,
                options,
                chosen: None,
                guard_cmd: None,
                change: None,
                release: None,
                asked: Instant::now(),
            }),
            roles::Line::Report(r) => {
                if let Some(i) = self.agent_index(who) {
                    self.agents[i].say(format!("report: {r}"));
                    let a = &mut self.agents[i];
                    a.last_report += &format!("{r}\n");
                }
                self.note(who, format!("report -> boss: {r}"));
            }
            roles::Line::Recommend {
                agent,
                vendor,
                model,
                why,
                source,
            } => {
                let Some(old) = self
                    .agents
                    .iter()
                    .find(|a| a.def.name == agent)
                    .map(|a| format!("{} {}", a.def.vendor, a.def.model))
                else {
                    self.note(
                        who,
                        format!("recommended a change for {agent}, who is not on the team"),
                    );
                    return;
                };
                if !matches!(vendor.as_str(), "claude" | "codex" | "gemini") {
                    self.note(who, format!("recommended {vendor}, which orda cannot run"));
                    return;
                }
                let src = source.map(|s| format!(" ({s})")).unwrap_or_default();
                self.ask(Question {
                    from: who.into(),
                    task: "models".into(),
                    text: format!("Move {agent} from {old} to {vendor} {model}? {why}{src}"),
                    options: vec![format!("switch to {model}"), "keep it".into()],
                    chosen: None,
                    guard_cmd: None,
                    change: Some(Change {
                        agent,
                        vendor,
                        model,
                    }),
                    release: None,
                    asked: Instant::now(),
                });
            }
            roles::Line::Hold { commit, reason } => {
                self.note(who, format!("hold on {commit}: {reason}"));
                self.holds.retain(|h| h.0 != commit);
                self.holds.push((commit, reason, who.into()));
            }
            roles::Line::Release(commit) => {
                self.holds
                    .retain(|h| !(h.0.starts_with(&commit) || commit.starts_with(&h.0)));
                self.note(who, format!("released {commit}"));
                let known: Vec<String> = self
                    .checks
                    .keys()
                    .filter(|h| h.starts_with(&commit) || commit.starts_with(h.as_str()))
                    .cloned()
                    .collect();
                for h in known {
                    self.maybe_merge(&h);
                }
            }
            roles::Line::Finished(summary) => {
                self.note(who, format!("finished the task: {summary}"));
                if self.agent_index("anchor").is_none() || self.flow.is_none() {
                    self.finish_task();
                }
                if let Some((Stage::Building, task)) = self.flow.clone()
                    && let Some(k) = self.agents.iter().position(|a| a.def.role == "anchor")
                {
                    self.flow = Some((Stage::Verifying, task.clone()));
                    self.verify_from = self.messages.last().map_or(1, |m| m.id + 1);
                    self.note("anchor", "checking every criterion has evidence");
                    self.queue_task(k, format!("The boss reports this task finished. Check every acceptance criterion of it in SPEC.md has evidence.\n\nThe task:\n{task}\n\nThe boss's summary:\n{summary}"));
                }
                // the map follows the code
                if let Some(c) = self.agents.iter().position(|a| a.def.role == "curator") {
                    self.queue_task(
                        c,
                        "The team finished a task. Bring MAP.md up to date with what changed."
                            .into(),
                    );
                }
            }
            roles::Line::Save { scope, text } => {
                if self.savings.iter().filter(|r| r.active).count() >= savings::MAX_ACTIVE {
                    self.note(
                        who,
                        format!(
                            "rule not added, {} are active already: {text}",
                            savings::MAX_ACTIVE
                        ),
                    );
                    return;
                }
                if let Some(id) =
                    savings::add(&mut self.savings, &scope, &text, &stats::runs(), now_unix())
                {
                    self.note(who, format!("rule {id} for {scope}: {text}"));
                    if !self.demo_mode {
                        savings::store(&self.savings);
                    }
                }
            }
            roles::Line::Drop(id) => {
                if savings::drop(&mut self.savings, &id, &format!("dropped by {who}")) {
                    self.note(who, format!("dropped rule {id}"));
                    if !self.demo_mode {
                        savings::store(&self.savings);
                    }
                }
            }
            roles::Line::Lesson(text) => {
                self.note(who, format!("lesson: {text}"));
                if !self.demo_mode {
                    add_lesson(&self.cwd, &text);
                }
            }
            roles::Line::Learned { what, source } => {
                let entry = match self.web.iter().position(|w| w.who == who) {
                    Some(i) => &mut self.web[i],
                    None => {
                        self.web.insert(
                            0,
                            Web {
                                who: who.into(),
                                query: "(from its own reading)".into(),
                                sources: vec![],
                                learned: None,
                                live: false,
                            },
                        );
                        &mut self.web[0]
                    }
                };
                entry.learned = Some(what);
                entry.sources.extend(source);
            }
        }
    }

    pub fn agent_event(&mut self, i: usize, ev: AgentEvent) {
        let Some(a) = self.agents.get_mut(i) else {
            return;
        };
        let name = a.def.name.clone();
        let vendor = a.def.vendor.clone();
        match ev {
            AgentEvent::Started { model } => {
                a.status = Status::Running;
                a.now = "thinking".into();
                if let Some(m) = model {
                    self.note(&name, format!("started on {m}"));
                }
            }
            AgentEvent::Text(t) => {
                let (cards, rest) = roles::cards(&t);
                let mut said = vec![];
                for line in rest.iter().filter(|l| !l.trim().is_empty()) {
                    match roles::protocol(line) {
                        Some(p) => said.push(p),
                        None => {
                            a.say(line.trim());
                            a.journal.push(format!("said: {}", short(line, 160)));
                        }
                    }
                }
                a.now = "writing".into();
                let task = a.task.clone();
                for p in said {
                    self.protocol(&name, &task, p);
                }
                for c in cards {
                    self.send(&name, c);
                }
            }
            AgentEvent::ToolCall { tool, detail } => {
                a.journal.push(format!("{tool} {detail}"));
                a.tools += 1;
                a.now = short(&format!("{tool} {detail}"), 60);
                a.status = Status::Running;
                if matches!(tool.as_str(), "Bash" | "shell") {
                    let verdict = guard::check(&self.cfg.guard, &detail);
                    if verdict != Verdict::Allow {
                        // claude runs are stopped by `orda hook`; codex stays inside its sandbox
                        let why = match (verdict, vendor.as_str()) {
                            (Verdict::Deny, "codex") => "codex sandbox: only its own folder",
                            (Verdict::Deny, _) => "blocked by the guard",
                            _ => "waiting for your yes",
                        };
                        self.guard_events.insert(
                            0,
                            GuardEvent {
                                at: clock()[..5].into(),
                                who: name.clone(),
                                cmd: detail.clone(),
                                verdict,
                                why: why.into(),
                            },
                        );
                        if verdict == Verdict::Ask {
                            self.ask(Question {
                                from: name.clone(),
                                task: "guard".into(),
                                text: format!("Allow `{}`?", short(&detail, 80)),
                                options: vec!["allow once".into(), "no".into()],
                                chosen: None,
                                guard_cmd: Some(detail.clone()),
                                change: None,
                                release: None,
                                asked: Instant::now(),
                            });
                        }
                    }
                }
            }
            AgentEvent::ToolResult { ok, detail } => {
                a.journal
                    .push(format!("  {} {detail}", if ok { "->" } else { "failed:" }));
                if !detail.is_empty() {
                    a.say(format!("{} {detail}", if ok { "->" } else { "x" }));
                }
            }
            AgentEvent::WebSearch { query } => {
                a.status = Status::Searching;
                a.now = short(&format!("web: {query}"), 60);
                for w in self.web.iter_mut().filter(|w| w.who == name) {
                    w.live = false;
                }
                self.web.insert(
                    0,
                    Web {
                        who: name.clone(),
                        query,
                        sources: vec![],
                        learned: None,
                        live: true,
                    },
                );
            }
            AgentEvent::Context(n) => a.context = n,
            AgentEvent::Tokens { input, output } => {
                a.tokens_in += input;
                a.tokens_out += output;
            }
            AgentEvent::Limit {
                window,
                used_pct,
                resets_at,
            } => limits::update(&mut self.limits, &vendor, &window, used_pct, resets_at),
            AgentEvent::Done { ok, cost_usd } => {
                let run = a.finish_run(ok);
                let def = a.def.clone();
                let back = a.return_pending
                    || a.primary
                        .as_ref()
                        .is_some_and(|p| !self.blocked.contains_key(&p.vendor));
                a.status = if ok { Status::Done } else { Status::Failed };
                a.now = if ok { "done".into() } else { "failed".into() };
                for w in self.web.iter_mut().filter(|w| w.who == name) {
                    w.live = false;
                }
                let cost = cost_usd.map(|c| format!(", ${c:.2}")).unwrap_or_default();
                self.note(&name, format!("finished{cost}"));
                if back {
                    self.agents[i].return_pending = false;
                    self.route(i);
                }
                if let Some(id) = self.agents[i].inbox.pop_front() {
                    self.deliver_now(i, id);
                }
                if let Some(r) = run {
                    stats::record(&def, &r);
                    self.after_run(i, &def, ok);
                }
                let goal = self.agents[i].goal.clone();
                self.check_done(&def.name, &goal);
                self.next_queued(i);
            }
            AgentEvent::Error(e) => {
                if let Some(r) = a.finish_run(false) {
                    stats::record(&a.def, &r);
                }
                a.status = Status::Failed;
                a.now = "failed".into();
                a.say(e.clone());
                self.note(&name, format!("error: {e}"));
                self.report_failure(i, &e);
                self.next_queued(i);
            }
            AgentEvent::ModelUnavailable(msg) => self.model_unavailable(i, &msg),
            AgentEvent::LimitHit { resets_at, message } => self.limit_hit(i, resets_at, &message),
            AgentEvent::Unknown(e) => self.note(&name, format!("unknown event: {e}")),
        }
    }
}

pub fn clock() -> String {
    let t = (now_unix() + utc_offset()).rem_euclid(86400);
    format!("{:02}:{:02}:{:02}", t / 3600, t % 3600 / 60, t % 60)
}

/// "15:05" for a unix time, in local time.
pub fn hhmm(at: i64) -> String {
    let t = (at + utc_offset()).rem_euclid(86400);
    format!("{:02}:{:02}", t / 3600, t % 3600 / 60)
}

pub fn utc_offset() -> i64 {
    // ponytail: read once at start, a DST switch mid-session shows the old hour
    static OFFSET: std::sync::OnceLock<i64> = std::sync::OnceLock::new();
    *OFFSET.get_or_init(|| {
        let out = std::process::Command::new("date").arg("+%z").output();
        let z = out
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_default();
        let n: i64 = z.get(1..).and_then(|d| d.parse().ok()).unwrap_or(0);
        let secs = n / 100 * 3600 + n % 100 * 60;
        if z.starts_with('-') { -secs } else { secs }
    })
}

pub fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn answers_paths(cwd: &str) -> [(std::path::PathBuf, Scope); 2] {
    [
        (
            config::home().join(".local/share/orda/answers.jsonl"),
            Scope::All,
        ),
        (
            std::path::Path::new(cwd).join(".orda/answers.jsonl"),
            Scope::Project,
        ),
    ]
}

fn load_answers(cwd: &str) -> Vec<Remembered> {
    let mut out = vec![];
    for (path, scope) in answers_paths(cwd) {
        for line in std::fs::read_to_string(path).unwrap_or_default().lines() {
            if let Some(text) = serde_json::from_str::<serde_json::Value>(line)
                .ok()
                .and_then(|v| v["text"].as_str().map(String::from))
            {
                out.insert(0, Remembered { text, scope });
            }
        }
    }
    out
}

fn save_answer(cwd: &str, text: &str, scope: Scope) {
    let Some((path, _)) = answers_paths(cwd).into_iter().find(|(_, s)| *s == scope) else {
        return;
    };
    let _ = std::fs::create_dir_all(path.parent().unwrap());
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(f, "{}", serde_json::json!({ "text": text }));
    }
}

fn today() -> String {
    let out = std::process::Command::new("date").arg("+%F").output();
    out.map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

/// The prompt for a model that takes over a task another model started.
fn handoff(goal: &str, journal: &[String], from: &str, why: &str) -> String {
    let done = if journal.is_empty() {
        "It had not done anything yet.".to_string()
    } else {
        let start = journal.len().saturating_sub(80);
        journal[start..]
            .iter()
            .map(|l| format!("- {l}"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    format!(
        "{goal}\n\n---\n\n## You are taking over this task\n\n{from} was working on it and had to stop: {why}. \
         Its changes are still on disk: run `git status` and `git diff` first. Do not redo finished work; carry on from where it stopped.\n\n\
         What it did, oldest first:\n\n{done}\n"
    )
}

/// Append one of ripple's lessons to the project's LESSONS.md.
fn add_lesson(cwd: &str, text: &str) {
    let path = std::path::Path::new(cwd).join("LESSONS.md");
    let fresh = !path.exists();
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        if fresh {
            let _ = writeln!(
                f,
                "# Lessons\n\nRules ripple wrote after catching a change that broke something else. orda gives them to every agent at the start of every run.\n"
            );
        }
        let _ = writeln!(f, "- {text} ({})", today());
    }
}

/// "/home/me/dev/x" as "~/dev/x".
pub fn tilde(path: &str) -> String {
    let home = config::home().display().to_string();
    match path.strip_prefix(&home) {
        Some(rest) if !home.is_empty() => format!("~{rest}"),
        _ => path.to_string(),
    }
}

/// Every log line also goes to ~/.local/share/orda/logs/<project>.log, to read after a session.
fn log_line(cwd: &str, who: &str, text: &str) {
    let name = cwd.rsplit('/').next().unwrap_or("orda");
    let dir = stats::dir().join("logs");
    let _ = std::fs::create_dir_all(&dir);
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join(format!("{name}.log")))
    {
        let _ = writeln!(f, "{} {} {who:<11} {text}", today(), clock());
    }
}

/// A short footer for an agent's card: what this run is about.
fn task_label(goal: &str) -> String {
    if let Some(rest) = goal.strip_prefix("New commit ") {
        return format!("checking {}", rest.split_whitespace().next().unwrap_or(""));
    }
    if let Some(rest) = goal.strip_prefix("You have a message from ") {
        // "boss (#50). The whole conversation..."
        let from = rest.split_whitespace().next().unwrap_or("");
        let id = rest.split(['(', ')']).nth(1).unwrap_or("");
        return format!("{id} from {from}");
    }
    short(goal.lines().next().unwrap_or(goal), 40)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limit_hit_switches_waits_and_comes_back() {
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let mut app = App::new(true, tx);
        let i = app.agent_index("builder").unwrap();
        assert_eq!(app.agents[i].def.vendor, "codex");
        app.agents[i].goal = "add the codex adapter".into();
        app.agents[i].journal = vec!["edit src/x.rs".into()];

        // codex runs out: same task moves to opus, the handoff carries what was done
        app.limit_hit(i, Some(now_unix() + 600), "You've hit your usage limit");
        let a = &app.agents[i];
        assert_eq!(
            (a.def.vendor.as_str(), a.def.model.as_str()),
            ("claude", "opus")
        );
        assert_eq!(a.primary.as_ref().unwrap().model, "gpt-6-sol");
        assert_eq!(a.status, Status::Running);
        assert!(
            app.log
                .iter()
                .any(|l| { l.text == "switched gpt-6-sol -> opus (claude)" })
        );
        let h = handoff(
            &a.goal,
            &a.journal,
            "gpt-6-sol (codex)",
            "its plan limit ran out",
        );
        assert!(
            h.starts_with("add the codex adapter")
                && h.contains("- edit src/x.rs")
                && h.contains("git status")
        );

        // claude runs out too: nothing left, the builder waits
        app.limit_hit(i, Some(now_unix() + 300), "claude five_hour limit reached");
        assert!(app.agents[i].waiting);
        assert_eq!(app.agents[i].status, Status::Queued);

        // both reset: the builder resumes on its own model
        for t in app.blocked.values_mut() {
            *t = now_unix() - 1;
        }
        app.check_resets();
        let a = &app.agents[i];
        assert_eq!(
            (a.def.vendor.as_str(), a.def.model.as_str()),
            ("codex", "gpt-6-sol")
        );
        assert!(a.primary.is_none() && !a.waiting);
        assert_eq!(a.status, Status::Running);
    }

    #[test]
    fn busy_agent_goes_back_after_its_run() {
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let mut app = App::new(true, tx);
        let i = app.agent_index("builder").unwrap();
        app.limit_hit(i, Some(now_unix() + 600), "usage limit");
        *app.blocked.get_mut("codex").unwrap() = now_unix() - 1;
        app.check_resets();
        assert_eq!(
            app.agents[i].def.model, "opus",
            "a running agent finishes its run first"
        );
        app.agent_event(
            i,
            AgentEvent::Done {
                ok: true,
                cost_usd: None,
            },
        );
        assert_eq!(app.agents[i].def.model, "gpt-6-sol");
        assert!(
            app.log
                .iter()
                .any(|l| l.text == "back on gpt-6-sol, switched from opus")
        );
    }

    #[test]
    fn a_new_commit_with_a_key_is_held() {
        let dir = std::env::temp_dir().join(format!("orda-scan-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let git = |args: &[&str]| {
            std::process::Command::new("git")
                .arg("-C")
                .arg(&dir)
                .args(args)
                .output()
                .unwrap();
        };
        git(&["init", "-q", "-b", "main"]);
        // assembled here so no real-looking token sits in the source
        let token = format!("xoxb-{}", "9Zq4".repeat(8));
        std::fs::write(dir.join("conf.toml"), format!("token = \"{token}\"\n")).unwrap();
        git(&["add", "."]);
        git(&[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@example.com",
            "commit",
            "-qm",
            "add conf",
        ]);

        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let mut app = App::new(true, tx);
        app.demo_mode = false;
        app.cfg.watch.commits = false;
        app.cfg.notify.desktop = false;
        app.cwd = dir.display().to_string();
        app.holds.clear();
        app.git = git::snapshot(&app.cwd);
        app.seen.clear();
        app.watch_commits();
        let hash = app.git.as_ref().unwrap().commits[0].hash.clone();
        assert!(
            app.holds
                .iter()
                .any(|h| h.0 == hash && h.1.contains("Slack token in conf.toml:1")),
            "{:?}",
            app.holds
        );
        assert!(
            app.questions
                .iter()
                .any(|q| q.release.as_deref() == Some(hash.as_str()))
        );
        app.watch_commits();
        assert_eq!(
            app.holds.iter().filter(|h| h.0 == hash).count(),
            1,
            "a commit is scanned once"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn anchor_brackets_the_boss() {
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let mut app = App::new(true, tx);
        let (k, b) = (
            app.agent_index("anchor").unwrap(),
            app.agent_index("boss").unwrap(),
        );
        app.flow = Some((Stage::Criteria, "add a config command".into()));
        app.agents[k].last_report = "1. orda config prints the path\n".into();
        let def = app.agents[k].def.clone();
        app.after_run(k, &def, true);
        assert_eq!(app.flow.as_ref().map(|f| f.0), Some(Stage::Building));
        assert!(
            app.agents[b]
                .goal
                .contains("## Acceptance criteria from anchor")
                && app.agents[b]
                    .goal
                    .contains("1. orda config prints the path")
        );
        app.agents[b].status = Status::Done;
        app.agents[k].status = Status::Done;
        // a boss run that only hands out work does not end the task
        let def = app.agents[b].def.clone();
        app.after_run(b, &def, true);
        assert_eq!(app.flow.as_ref().map(|f| f.0), Some(Stage::Building));
        app.agent_event(b, AgentEvent::Text("FINISHED: config command works".into()));
        assert_eq!(app.flow.as_ref().map(|f| f.0), Some(Stage::Verifying));
        assert!(
            app.agents[k]
                .goal
                .starts_with("The boss reports this task finished")
        );
        let c = app.agent_index("curator").unwrap();
        assert!(
            app.agents[c].goal.contains("MAP.md"),
            "the map follows each finished task"
        );
    }

    #[test]
    fn a_builder_commit_is_checked_then_merged() {
        let dir = std::env::temp_dir().join(format!("orda-flow-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        work::ensure_repo(&dir).unwrap();

        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let mut app = App::new(true, tx);
        app.demo_mode = false;
        app.cfg.notify.desktop = false;
        app.cwd = dir.display().to_string();
        app.holds.clear();
        app.git = git::snapshot(&app.cwd);
        app.seen = git::recent_all(&app.cwd).into_iter().map(|c| c.0).collect();
        // everyone is busy, so work is queued instead of starting a real run
        for a in app.agents.iter_mut() {
            a.status = Status::Running;
        }

        let main = app.git.as_ref().unwrap().branch.clone();
        let wt = work::prepare(&dir, "builder", work::Job::Task { main: &main }).unwrap();
        std::fs::write(wt.join("app.py"), "print('hi')\n").unwrap();
        let g = |args: &[&str]| {
            std::process::Command::new("git")
                .arg("-C")
                .arg(&wt)
                .args(args)
                .output()
                .unwrap()
        };
        g(&["add", "."]);
        g(&[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@example.com",
            "commit",
            "-qm",
            "add app",
        ]);
        let hash = String::from_utf8(g(&["rev-parse", "--short", "HEAD"]).stdout)
            .unwrap()
            .trim()
            .to_string();

        app.watch_commits();
        let (branch, pending) = app
            .checks
            .get(&hash)
            .cloned()
            .expect("the commit is being checked");
        assert_eq!(branch, "wt/builder");
        let mut want = vec!["tester", "ripple", "aegis", "referee", "reviewer"];
        want.sort();
        let mut got = pending.clone();
        got.sort();
        assert_eq!(
            got, want,
            "customs skips commits without a manifest, the builder does not check itself"
        );

        let boss = app.agent_index("boss").unwrap();
        let goal = format!("New commit {hash} on wt/builder: \"add app\".");
        for who in ["tester", "ripple", "aegis", "reviewer"] {
            app.check_done(who, &goal);
        }
        assert!(
            app.agents[boss].queue.is_empty(),
            "no merge while the referee is still checking"
        );
        // the referee holds it; its run ends; still no merge
        app.holds
            .push((hash.clone(), "test bent".into(), "referee".into()));
        app.check_done("referee", &goal);
        assert!(app.agents[boss].queue.is_empty(), "no merge while held");
        let r = app.agent_index("referee").unwrap();
        app.agent_event(r, AgentEvent::Text(format!("RELEASE: {hash}")));
        let task = app.agents[boss]
            .queue
            .back()
            .expect("the boss is asked to merge");
        assert!(task.contains("git merge --no-ff wt/builder") && task.contains(&hash));

        let _ = std::process::Command::new("git")
            .arg("-C")
            .arg(&dir)
            .args(["worktree", "remove", "-f", &wt.display().to_string()])
            .output();
        let _ = std::fs::remove_dir_all(work::base(&dir));
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn real_app() -> App {
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let mut app = App::new(true, tx);
        app.demo_mode = false;
        app.cfg.notify.desktop = false;
        app.holds.clear();
        app.messages.clear();
        app.task.clear();
        for a in app.agents.iter_mut() {
            a.status = Status::Idle;
            a.queue.clear();
        }
        app
    }

    fn card(to: &str, kind: &str, commit: &str) -> roles::Card {
        let mut fields = vec![("title".to_string(), "t".to_string())];
        if !commit.is_empty() {
            fields.push(("commit".into(), commit.into()));
        }
        roles::Card {
            to: to.into(),
            re: None,
            kind: kind.into(),
            fields,
        }
    }

    #[test]
    fn thanks_do_not_start_runs() {
        // the hello-orda run: referee and builder-2 woke each other with done and note cards ~20 times
        let mut app = real_app();
        let r = app.agent_index("referee").unwrap();
        for kind in ["done", "note"] {
            let id = app.send("builder-2", card("referee", kind, "")).unwrap();
            app.deliver(id);
        }
        assert_eq!(app.agents[r].status, Status::Idle, "no run started");
        assert!(app.agents[r].inbox.is_empty());
        assert_eq!(
            app.agents[r].mailbox.len(),
            2,
            "read at its next run instead"
        );
    }

    #[test]
    fn the_referee_verifies_a_commit_once() {
        let mut app = real_app();
        let r = app.agent_index("referee").unwrap();
        app.agents[r].status = Status::Running; // queue instead of starting a real run
        app.send("builder-2", card("boss", "done", "c82501b"));
        app.send("builder-2", card("tester", "done", "c82501b"));
        app.send("builder-2", card("boss", "fixed", "c82501b"));
        app.send("builder-2", card("boss", "done", ""));
        assert_eq!(app.agents[r].queue.len(), 1);
        assert!(app.agents[r].queue[0].starts_with("New commit c82501b"));
    }

    #[test]
    fn a_refused_model_moves_to_the_fallback() {
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let mut app = App::new(true, tx);
        let b = app.agent_index("builder").unwrap();
        app.agent_event(
            b,
            AgentEvent::ModelUnavailable("The 'gpt-6-sol' model is not supported".into()),
        );
        let a = &app.agents[b];
        assert_eq!(
            (a.def.vendor.as_str(), a.def.model.as_str()),
            ("claude", "opus")
        );
        assert!(app.blocked.contains_key("codex:gpt-6-sol"));
        assert!(
            !app.blocked.contains_key("codex"),
            "other codex models stay usable"
        );
    }

    #[test]
    fn a_failure_reaches_the_boss() {
        let mut app = real_app();
        let b = app.agent_index("builder").unwrap();
        app.agents[b].goal = "write todo.py".into();
        app.agents[b].status = Status::Running;
        app.agent_event(b, AgentEvent::Error("exited with an error".into()));
        let m = app.messages.last().expect("a card went out");
        assert_eq!(
            (m.from.as_str(), m.to.as_str(), m.card.kind.as_str()),
            ("builder", "boss", "failure")
        );
        assert!(mail::wakes("failure"));
    }

    #[test]
    fn the_header_says_where_the_task_is() {
        let mut app = real_app();
        assert_eq!(app.task_state().0, "no task yet");
        app.task = "build x".into();
        app.agents[0].status = Status::Running;
        assert_eq!(app.task_state().0, "working, 1 agent");
        app.agents[0].status = Status::Done;
        app.task_done = Some(Instant::now());
        assert_eq!(app.task_state().0, "finished");
    }

    #[test]
    fn card_footers_are_short() {
        assert_eq!(
            task_label("New commit 8151b27 on wt/builder-2: \"x\"."),
            "checking 8151b27"
        );
        assert_eq!(
            task_label("You have a message from boss (#50). The whole conversation so far"),
            "#50 from boss"
        );
        assert_eq!(
            task_label("Write the acceptance criteria\nfor this"),
            "Write the acceptance criteria"
        );
    }

    /// Look at a real project without running anything: `ORDA_DIR=~/dev/x cargo test peek -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn peek() {
        let Ok(dir) = std::env::var("ORDA_DIR") else {
            return;
        };
        let mut app = real_app();
        app.cwd = dir;
        app.git = git::snapshot(&app.cwd);
        let mut term = ratatui::Terminal::new(ratatui::backend::TestBackend::new(196, 54)).unwrap();
        term.draw(|f| crate::ui::draw(f, &app)).unwrap();
        let buf = term.backend().buffer();
        for y in 0..20 {
            println!(
                "{}",
                (100..196)
                    .map(|x| buf[(x, y)].symbol().to_string())
                    .collect::<String>()
            );
        }
    }
}
