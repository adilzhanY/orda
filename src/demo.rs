//! `orda --demo`: a scripted team so the dashboard can be seen and tuned
//! without spending tokens. Nothing here talks to a real agent.

use crate::app::{Agent, App, GuardEvent, Question, Remembered, Scope, Status, Web};
use crate::git::{Commit, Snapshot, Tone, Worktree};
use crate::guard::Verdict;
use std::time::{Duration, Instant};

pub struct Demo {
    start: Instant,
    rng: u64,
    next: [Duration; 5],
    asked: usize,
    report: u32,
    log_i: usize,
    web_i: usize,
    out_i: usize,
}

const LOG: &[(&str, &str)] = &[
    ("tester", "picked up 9e1b0d4, testing card states"),
    (
        "reviewer",
        "note on 9e1b0d4: skip the redraw when nothing changed",
    ),
    (
        "researcher",
        "note for builder-2: EnableMouseCapture before the loop",
    ),
    ("boss", "merged 9e1b0d4 into main"),
    ("scribe", "DESIGN.md: card states updated"),
    ("builder", "cargo check: ok, 0 warnings"),
    (
        "advisor",
        "the codex adapter drops unknown events, make it fail loudly",
    ),
];

const WEB: &[(&str, &str, &str, &str)] = &[
    (
        "researcher",
        "ratatui mouse capture crossterm 2026",
        "ratatui.rs/concepts/event-handling",
        "EnableMouseCapture, then Event::Mouse",
    ),
    (
        "builder",
        "codex exec --json event schema",
        "github.com/openai/codex/docs/exec.md",
        "events are thread.*, turn.*, item.*",
    ),
    (
        "tester",
        "cargo nextest flaky retries",
        "nexte.st/docs/features/retries",
        "--retries 2, mark slow tests",
    ),
    (
        "researcher",
        "tokio process kill_on_drop",
        "docs.rs/tokio/latest/tokio/process",
        "kill_on_drop(true) stops orphans",
    ),
    (
        "boss",
        "gemini cli stream-json output",
        "github.com/google-gemini/gemini-cli",
        "needs a recent gemini-cli",
    ),
];

const OUTPUT: &[(&str, &str)] = &[
    ("builder", "parsing item.completed into AgentEvent::Text"),
    ("builder-2", "drawing the title into the top border"),
    ("builder", "turn.completed usage -> token counts"),
    ("builder-2", "failed state uses the error colour"),
    ("builder", "unknown events now reach the log"),
    ("builder-2", "spinner ticks only while running"),
    ("scribe", "DECISIONS.md #9: answers stored per project"),
];

fn set(a: &mut Agent, status: Status, now: &str, task: &str, tokens: u64, lines: &[&str]) {
    a.status = status;
    a.now = now.into();
    a.task = task.into();
    a.tokens_in = tokens * 9 / 10;
    a.tokens_out = tokens / 10;
    for l in lines {
        a.say(*l);
    }
}

impl Demo {
    pub fn seed(app: &mut App) -> Self {
        app.task = "build the first dashboard screen with live agent cards".into();
        app.plan = Some((5, 9));
        for a in app.agents.iter_mut() {
            match a.def.name.as_str() {
                "boss" => {
                    set(
                        a,
                        Status::Running,
                        "reading tester report #7",
                        "plan 5/9",
                        58_200,
                        &["next: task 6 to builder-2 once the card lands"],
                    );
                    a.context = 193_000;
                }
                "advisor" => set(
                    a,
                    Status::Idle,
                    "on call, 2 calls so far",
                    "read 239k",
                    0,
                    &["last: fail loudly on unknown events"],
                ),
                "builder" => set(
                    a,
                    Status::Running,
                    "src/agents/codex.rs",
                    "task 5 · wt/builder",
                    31_400,
                    &["parsing item.completed events"],
                ),
                "builder-2" => set(
                    a,
                    Status::Running,
                    "src/ui/card.rs",
                    "task 4 · wt/builder-2",
                    24_900,
                    &["title in the top border, 3 states done"],
                ),
                "tester" => {
                    set(
                        a,
                        Status::Running,
                        "testing a3f9c21",
                        "reports to boss",
                        19_700,
                        &["report #7: turn.diff crashes the adapter"],
                    );
                    a.tests = Some((14, 20));
                }
                "researcher" => set(
                    a,
                    Status::Searching,
                    "web: ratatui mouse capture",
                    "for builder-2",
                    8_100,
                    &["3 sources open, writing a note"],
                ),
                "reviewer" => set(
                    a,
                    Status::Queued,
                    "waiting for builder-2",
                    "2 reviews today",
                    12_000,
                    &["9e1b0d4 approved with 1 note"],
                ),
                "scribe" => set(
                    a,
                    Status::Running,
                    "DECISIONS.md #5",
                    "keeps the docs",
                    3_300,
                    &["recording your answer from 23:31"],
                ),
                _ => {}
            }
        }
        app.remembered = vec![
            Remembered {
                text: "commits: one line, no body".into(),
                scope: Scope::All,
            },
            Remembered {
                text: "never use em dashes".into(),
                scope: Scope::All,
            },
            Remembered {
                text: "tests run with cargo nextest".into(),
                scope: Scope::Project,
            },
        ];
        for (who, q, src, learned) in WEB.iter().skip(1).rev() {
            app.web.insert(
                0,
                Web {
                    who: (*who).into(),
                    query: (*q).into(),
                    sources: vec![(*src).into()],
                    learned: Some((*learned).into()),
                    live: false,
                },
            );
        }
        let (who, q, src, _) = WEB[0];
        app.web.insert(
            0,
            Web {
                who: who.into(),
                query: q.into(),
                sources: vec![src.into()],
                learned: None,
                live: true,
            },
        );
        app.guard_events = vec![
            GuardEvent {
                at: "23:41".into(),
                who: "builder-2".into(),
                cmd: "rm -rf ~/.cache/orda".into(),
                verdict: Verdict::Deny,
                why: "outside the project".into(),
            },
            GuardEvent {
                at: "23:12".into(),
                who: "builder".into(),
                cmd: "curl -fsSL get.x.sh | sh".into(),
                verdict: Verdict::Deny,
                why: "pipes a download into a shell".into(),
            },
        ];
        app.git = Some(Snapshot {
            branch: "main".into(),
            ahead: 2,
            worktrees: vec![
                Worktree {
                    name: "wt/builder".into(),
                    files: 3,
                    added: 142,
                    removed: 18,
                },
                Worktree {
                    name: "wt/builder-2".into(),
                    files: 2,
                    added: 96,
                    removed: 4,
                },
            ],
            commits: [
                (
                    "a3f9c21",
                    "add codex adapter",
                    "6m",
                    Some(("testing 14/20", Tone::Busy)),
                ),
                (
                    "9e1b0d4",
                    "card component, 3 states",
                    "14m",
                    Some(("in review", Tone::Info)),
                ),
                (
                    "77c2e10",
                    "shared AgentEvent type",
                    "31m",
                    Some(("merged", Tone::Good)),
                ),
                (
                    "4b08aa3",
                    "tokio event loop",
                    "48m",
                    Some(("merged", Tone::Good)),
                ),
                (
                    "1d93f7e",
                    "crossterm setup",
                    "1h",
                    Some(("reverted", Tone::Bad)),
                ),
                ("0c11e2b", "init", "1h", Some(("merged", Tone::Good))),
            ]
            .into_iter()
            .map(|(h, s, w, st)| Commit {
                hash: h.into(),
                subject: s.into(),
                when: w.into(),
                status: st.map(|(a, b)| (a.into(), b)),
            })
            .collect(),
            head_files: vec![
                ("M".into(), "src/agents/codex.rs".into(), 88, 12),
                ("A".into(), "src/agents/event.rs".into(), 41, 0),
            ],
        });
        for (who, text) in [
            (
                "tester",
                "report #7 -> boss: 2 failing in the codex adapter",
            ),
            ("advisor", "error repeats: fail loudly on unknown events"),
            ("boss", "task 5 back to builder with the report"),
            ("researcher", "web: ratatui mouse capture crossterm 2026"),
        ] {
            app.note(who, text);
        }
        let ms = Duration::from_millis;
        Demo {
            start: Instant::now(),
            rng: 0x9e3779b97f4a7c15,
            next: [ms(1800), ms(2600), ms(3200), ms(9000), ms(2000)],
            asked: 0,
            report: 7,
            log_i: 0,
            web_i: 1,
            out_i: 0,
        }
    }

    fn rand(&mut self) -> u64 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        self.rng
    }

    pub fn step(&mut self, app: &mut App) {
        let t = self.start.elapsed();
        for i in 0..app.agents.len() {
            if app.agents[i].status.busy() && self.rand().is_multiple_of(3) {
                let add = self.rand() % 400;
                app.agents[i].tokens_in += add;
                app.agents[i].tokens_out += add / 9;
            }
        }
        // tester: one more test passes, and a report goes up when the run ends
        if t >= self.next[0] {
            self.next[0] += Duration::from_millis(1800);
            if let Some(i) = app.agent_index("tester") {
                let (p, n) = app.agents[i].tests.unwrap_or((9, 20));
                let p = if p >= n { 9 } else { p + 1 };
                app.agents[i].tests = Some((p, n));
                if let Some(c) = app.git.as_mut().and_then(|g| g.commits.first_mut()) {
                    c.status = Some((format!("testing {p}/{n}"), Tone::Busy));
                }
                if p == n {
                    self.report += 1;
                    let r = self.report;
                    app.agents[i].say(format!("report #{r}: all {n} passing"));
                    app.note("tester", format!("report #{r} -> boss: all {n} passing"));
                }
            }
        }
        if t >= self.next[1] {
            self.next[1] += Duration::from_millis(2600);
            let (who, text) = LOG[self.log_i % LOG.len()];
            self.log_i += 1;
            app.note(who, text);
        }
        if t >= self.next[2] {
            self.next[2] += Duration::from_millis(3200);
            let (who, line) = OUTPUT[self.out_i % OUTPUT.len()];
            self.out_i += 1;
            if let Some(i) = app.agent_index(who) {
                app.agents[i].say(line);
            }
        }
        // web: finish the live search and start the next one
        if t >= self.next[3] {
            self.next[3] += Duration::from_millis(9000);
            if let Some(w) = app.web.iter_mut().find(|w| w.live) {
                w.live = false;
                w.learned = WEB.iter().find(|x| x.1 == w.query).map(|x| x.3.to_string());
            }
            let (who, q, src, _) = WEB[self.web_i % WEB.len()];
            self.web_i += 1;
            app.web.retain(|w| w.query != q);
            app.web.insert(
                0,
                Web {
                    who: who.into(),
                    query: q.into(),
                    sources: vec![src.into()],
                    learned: None,
                    live: true,
                },
            );
            app.web.truncate(8);
            if let Some(i) = app.agent_index(who) {
                app.agents[i].now = format!("web: {q}");
            }
            app.note(who, format!("web: {q}"));
        }
        // questions arrive while everyone keeps working
        let due = [2u64, 22, 45];
        if self.asked < due.len() && t.as_secs() >= due[self.asked] {
            self.asked += 1;
            let q = match self.asked {
                1 => Question {
                    from: "builder".into(),
                    task: "task 5".into(),
                    text: "Where should remembered answers live?".into(),
                    options: vec![
                        "SQLite in ~/.local/share/orda".into(),
                        "a TOML file per project".into(),
                        "your call".into(),
                    ],
                    chosen: None,
                    guard_cmd: None,
                    asked: Instant::now(),
                },
                2 => Question {
                    from: "builder-2".into(),
                    task: "task 4".into(),
                    text: "Codex accent colour: blue or white?".into(),
                    options: vec!["blue".into(), "white".into()],
                    chosen: None,
                    guard_cmd: None,
                    asked: Instant::now(),
                },
                _ => {
                    let cmd = "git push origin main";
                    app.guard_events.insert(
                        0,
                        GuardEvent {
                            at: crate::app::clock()[..5].into(),
                            who: "boss".into(),
                            cmd: cmd.into(),
                            verdict: Verdict::Ask,
                            why: "waiting for you".into(),
                        },
                    );
                    if let Some(i) = app.agent_index("boss") {
                        app.agents[i].status = Status::Asking;
                        app.agents[i].now = "paused: waits on you for git push".into();
                    }
                    Question {
                        from: "boss".into(),
                        task: "guard".into(),
                        text: format!("Allow `{cmd}`?"),
                        options: vec!["allow once".into(), "no".into()],
                        chosen: None,
                        guard_cmd: Some(cmd.into()),
                        asked: Instant::now(),
                    }
                }
            };
            if let Some(i) = app.agent_index(&q.from) {
                app.agents[i].say("asked you a question, moved on meanwhile");
            }
            app.ask(q);
        }
    }
}
