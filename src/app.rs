use crate::agents::{self, AgentEvent, short};
use crate::config::{self, AgentDef, Config};
use crate::demo::Demo;
use crate::git::{self, Snapshot};
use crate::guard::{self, Verdict};
use crate::limits::{self, Service};
use crate::theme::Theme;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::collections::{HashMap, VecDeque};
use std::io::Write;
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::mpsc::UnboundedSender;

pub enum Msg {
    Key(KeyEvent),
    Redraw,
    Agent(usize, AgentEvent),
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
        }
    }

    pub fn tokens(&self) -> u64 {
        self.tokens_in + self.tokens_out
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
    pub asked: Instant,
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
    pub project: String,
    cwd: String,
    timers: [Instant; 4],
    pub quit: bool,
    tx: UnboundedSender<Msg>,
}

const SECOND: Duration = Duration::from_secs(1);

impl App {
    pub fn new(demo: bool, tx: UnboundedSender<Msg>) -> Self {
        let (cfg, err) = config::load();
        let cwd = std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        let project = cwd.rsplit('/').next().unwrap_or("").to_string();
        let now = Instant::now();
        let mut app = Self {
            theme: Theme::build(&cfg.theme, &cfg.colors),
            agents: cfg.agents.iter().cloned().map(Agent::new).collect(),
            cfg,
            cfg_mtime: config::modified(),
            demo: None,
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
            project,
            cwd,
            timers: [now; 4],
            quit: false,
            tx,
        };
        if let Some(e) = err {
            app.note("orda", e);
        }
        if demo {
            app.demo = Some(Demo::seed(&mut app));
        } else {
            app.git = git::snapshot(&app.cwd);
            app.remembered = load_answers(&app.cwd);
            for a in app.agents.iter_mut().filter(|a| a.def.always_on) {
                a.status = Status::Queued;
                a.now = "waits for the first commit".into();
            }
            app.note("orda", "ready. press i and tell orda what to build");
        }
        app
    }

    pub fn note(&mut self, who: &str, text: impl Into<String>) {
        self.log.push_back(LogLine {
            at: clock(),
            who: who.into(),
            text: text.into(),
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
            Msg::Agent(i, ev) => self.agent_event(i, ev),
        }
    }

    /// Returns true when the screen needs a redraw.
    pub fn tick(&mut self) -> bool {
        self.frame += 1;
        let now = Instant::now();
        let mut dirty = false;
        if let Some(mut demo) = self.demo.take() {
            demo.step(self);
            self.demo = Some(demo);
            dirty = true;
        }
        if now - self.timers[0] >= SECOND {
            self.timers[0] = now;
            self.reload_config();
            dirty = true; // the elapsed clock moves every second
        }
        if now - self.timers[1] >= 2 * SECOND {
            self.timers[1] = now;
            self.sample_tokens();
        }
        if now - self.timers[2] >= 5 * SECOND && self.demo.is_none() {
            self.timers[2] = now;
            self.git = git::snapshot(&self.cwd);
        }
        if now - self.timers[3] >= 30 * SECOND {
            self.timers[3] = now;
            let live = std::mem::take(&mut self.limits);
            self.limits = limits::load();
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
        for a in &self.agents {
            *totals.entry(a.def.vendor.clone()).or_default() += a.tokens();
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
        // a guard question has no scope: the answer is for this one command
        if q.guard_cmd.is_some() {
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
        let text = format!("{}: {answer}", short(&q.text, 40).trim_end_matches('?'));
        if scope != Scope::Once && self.demo.is_none() {
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
        if self.demo.is_some() {
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
        let a = &mut self.agents[i];
        if a.status.busy() {
            self.note("orda", "the boss is still working on the last task");
            return;
        }
        a.status = Status::Running;
        a.now = "starting".into();
        a.task = short(&task, 40);
        a.output.clear();
        let d = a.def.clone();
        agents::spawn(
            &d.vendor,
            &d.model,
            &d.effort,
            &task,
            self.tx.clone(),
            Msg::Agent,
            i,
        );
    }

    fn agent_event(&mut self, i: usize, ev: AgentEvent) {
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
                for line in t.lines().filter(|l| !l.trim().is_empty()) {
                    a.say(line.trim());
                }
                a.now = "writing".into();
            }
            AgentEvent::ToolCall { tool, detail } => {
                a.now = short(&format!("{tool} {detail}"), 60);
                a.status = Status::Running;
                if matches!(tool.as_str(), "Bash" | "shell") {
                    let verdict = guard::check(&self.cfg.guard, &detail);
                    if verdict != Verdict::Allow {
                        // ponytail: shown only; enforcement needs PreToolUse hooks (claude) and an exec policy (codex)
                        self.guard_events.insert(
                            0,
                            GuardEvent {
                                at: clock()[..5].into(),
                                who: name.clone(),
                                cmd: detail.clone(),
                                verdict,
                                why: "seen only, enforcement comes next".into(),
                            },
                        );
                    }
                }
            }
            AgentEvent::ToolResult { ok, detail } => {
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
                a.status = if ok { Status::Done } else { Status::Failed };
                a.now = if ok { "done".into() } else { "failed".into() };
                for w in self.web.iter_mut().filter(|w| w.who == name) {
                    w.live = false;
                }
                let cost = cost_usd.map(|c| format!(", ${c:.2}")).unwrap_or_default();
                self.note(&name, format!("finished{cost}"));
            }
            AgentEvent::Error(e) => {
                a.status = Status::Failed;
                a.now = "failed".into();
                a.say(e.clone());
                self.note(&name, format!("error: {e}"));
            }
            AgentEvent::Unknown(e) => self.note(&name, format!("unknown event: {e}")),
        }
    }
}

pub fn clock() -> String {
    // ponytail: UTC offset read once at start, a DST switch mid-session shows the old hour
    static OFFSET: std::sync::OnceLock<i64> = std::sync::OnceLock::new();
    let off = *OFFSET.get_or_init(|| {
        let out = std::process::Command::new("date").arg("+%z").output();
        let z = out
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_default();
        let n: i64 = z.get(1..).and_then(|d| d.parse().ok()).unwrap_or(0);
        let secs = n / 100 * 3600 + n % 100 * 60;
        if z.starts_with('-') { -secs } else { secs }
    });
    let t = (now_unix() + off).rem_euclid(86400);
    format!("{:02}:{:02}:{:02}", t / 3600, t % 3600 / 60, t % 60)
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
