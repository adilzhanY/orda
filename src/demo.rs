//! `orda --demo`: a scripted team so the dashboard can be seen and tuned
//! without spending tokens. Nothing here talks to a real agent.

use crate::app::{Agent, App, Change, GuardEvent, Question, Remembered, Scope, Status, Web};
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
    /// 0: nothing yet, 1: builder hit its codex limit, 2: the limit reset, 3: back on codex
    limit_act: u8,
    last_frame: usize,
    /// Index into MAIL, and the start of the current pass through it.
    mail_i: usize,
    mail_from: Duration,
    threads: [Option<u32>; 9],
    scanned: bool,
    /// How many of the free-check and refresh scenes have played this pass.
    free_i: u8,
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

/// (second, sender, conversation, card). `{re}` becomes ` re #<first id>` of that conversation.
/// The script repeats every 140 seconds.
const MAIL: &[(u64, &str, usize, &str)] = &[
    (
        8,
        "tester",
        0,
        "MSG tester -> builder\nkind: bug\nseverity: high\ncommit: a3f9c21\ntitle: turn.diff event crashes the codex adapter\nrepro: cargo test codex::unknown_item\nexpected: AgentEvent::Unknown, the run keeps going\nactual: panic at src/vendors/codex.rs:48\ndone when: the test passes and a live run survives turn.diff\nEND",
    ),
    (
        15,
        "builder",
        0,
        "MSG builder -> tester{re}\nkind: fixed\ntitle: unknown codex items become AgentEvent::Unknown\ncommit: b71c0e2\nEND",
    ),
    (
        22,
        "tester",
        0,
        "MSG tester -> builder{re}\nkind: bug\nseverity: high\ntitle: item.updated still panics\nrepro: cargo test codex::item_updated\nexpected: no panic\nactual: panic at src/vendors/codex.rs:61\ndone when: both tests pass\nEND",
    ),
    (
        29,
        "builder",
        0,
        "MSG builder -> tester{re}\nkind: fixed\ntitle: item.updated handled the same way\ncommit: c40d9aa\nEND",
    ),
    (
        36,
        "tester",
        0,
        "MSG tester -> builder{re}\nkind: done\ntitle: verified, 22 of 22 passing\nEND",
    ),
    (
        40,
        "ripple",
        3,
        "HOLD: 9e1b0d4 | lr() now pads one column too far\nMSG ripple -> builder-2\nkind: bug\nseverity: high\ncommit: 9e1b0d4\ntitle: the card commit shifts the git and log panels by one column\nrepro: cargo test ripple_lr_width (passes on 9e1b0d4^, fails on 9e1b0d4)\nexpected: lr() returns exactly the width it is given\nactual: one column wider, every panel that calls lr() wraps its last column\ndone when: ripple_lr_width passes and every panel renders as on the parent\nEND\nMSG ripple -> tester\nkind: note\ntitle: 11 places call lr() and none had a width test; ripple_lr_width covers it now\nEND",
    ),
    (
        44,
        "reviewer",
        1,
        "MSG reviewer -> builder-2\nkind: review\ntitle: the card redraws every tick even when idle\nwhere: src/ui/card.rs:88\nsuggest: draw only when the agent changed\nEND",
    ),
    (
        49,
        "builder-2",
        3,
        "MSG builder-2 -> ripple{re}\nkind: fixed\ntitle: lr() fills to the width, not past it\ncommit: 3c81f0e\nEND",
    ),
    (
        51,
        "builder-2",
        1,
        "MSG builder-2 -> reviewer{re}\nkind: fixed\ntitle: redraw only on change\ncommit: 5e2f1b7\nEND",
    ),
    (
        55,
        "ripple",
        3,
        "RELEASE: 9e1b0d4\nMSG ripple -> builder-2{re}\nkind: done\ntitle: fix holds, every panel matches the parent\nEND\nLESSON: when you change a helper in ui/mod.rs, render every panel that calls it before you commit",
    ),
    (
        57,
        "reviewer",
        1,
        "MSG reviewer -> builder-2{re}\nkind: done\ntitle: approved\nEND",
    ),
    (
        60,
        "aegis",
        4,
        "HOLD: 4b08aa3 | the guard misses commands hidden in bash -c and eval\nMSG aegis -> builder\nkind: bug\nseverity: critical\ncommit: 4b08aa3\ntitle: deny rules can be walked around with bash -c, eval or $( )\nrepro: cargo test guard::tests::verdicts with bash -c \"rm -rf /\"\nexpected: Deny\nactual: Allow, the command runs\ndone when: commands inside quotes, $( ) and after sudo, env, eval, nohup or xargs are checked too\nEND",
    ),
    (
        67,
        "builder",
        4,
        "MSG builder -> aegis{re}\nkind: fixed\ntitle: the guard looks inside quotes, $( ) and wrappers\ncommit: 8f2d6a1\nEND",
    ),
    (
        72,
        "aegis",
        4,
        "RELEASE: 4b08aa3\nMSG aegis -> builder{re}\nkind: done\ntitle: the bypasses are blocked, the proof test passes\nEND\nLESSON: a deny list must also check commands inside quotes, $( ) and after wrappers like sudo, env and eval",
    ),
    (
        84,
        "bursar",
        5,
        "SAVE: all | Search with grep and open only the lines around the matches; read a whole file only when you will change most of it\nMSG bursar -> scout\nkind: note\ntitle: the tester spends 3.1 times the builder's tokens per run for the same success rate\nask: worth checking whether sonnet at low effort holds the tester's success rate\nEND\nREPORT: tokens per run down 18% this week; biggest cost is the tester rereading full test logs",
    ),
    (
        64,
        "tester",
        2,
        "MSG tester -> builder-2\nkind: bug\nseverity: medium\ntitle: spinner keeps turning after done\nrepro: orda --demo, wait for a run to finish\nexpected: the spinner stops\nactual: it keeps turning\ndone when: no spinner on a finished card\nEND",
    ),
    (
        70,
        "builder-2",
        2,
        "MSG builder-2 -> tester{re}\nkind: fixed\ntitle: spinner only while busy\ncommit: 91aa3c0\nEND",
    ),
    (
        76,
        "tester",
        2,
        "MSG tester -> builder-2{re}\nkind: bug\nseverity: medium\ntitle: still turns on the scout card\nrepro: press s in demo mode\nexpected: no spinner\nactual: spinner\ndone when: no spinner on any idle card\nEND",
    ),
    (
        82,
        "builder-2",
        2,
        "MSG builder-2 -> tester{re}\nkind: fixed\ntitle: idle cards never spin\ncommit: 0d7e2b4\nEND",
    ),
    (
        88,
        "tester",
        2,
        "MSG tester -> builder-2{re}\nkind: bug\nseverity: medium\ntitle: turns again after a fallback switch\nrepro: wait for the limit switch in demo\nexpected: no spinner\nactual: spinner after the switch\ndone when: no spinner after a switch\nEND",
    ),
    (
        96,
        "anchor",
        6,
        "MSG anchor -> boss\nkind: bug\nseverity: medium\ntitle: criteria 4 and 7 of task 5 have no evidence\nrepro: SPEC.md, task 5\nexpected: a test or an output for every criterion\nactual: 4, no test checks the fallback card title; 7, no check of the tabs below 170 columns\ndone when: both have a test that passes\nEND",
    ),
    (
        103,
        "referee",
        7,
        "HOLD: 5e2f1b7 | a test was loosened to pass\nMSG referee -> builder-2\nkind: bug\nseverity: high\ncommit: 5e2f1b7\ntitle: the card width test was loosened instead of fixing the code\nrepro: git show 5e2f1b7 -- src/ui/team.rs, then cargo test card_width\nexpected: the test still checks the exact width, assert_eq!(w, 40)\nactual: it became assert!(w > 0) and passes for any width\ndone when: the exact assert is back and passes\nEND\nLESSON: never loosen an assert to make a test pass; fix the code, or say why the old value was wrong",
    ),
    (
        109,
        "customs",
        8,
        "MSG customs -> builder\nkind: bug\nseverity: high\ncommit: 3c81f0e\ntitle: ratatui-flexbox does not exist on crates.io\nrepro: curl https://crates.io/api/v1/crates/ratatui-flexbox gives 404\nexpected: a real crate, or none\nactual: an invented name; ratatui already has Flex in ratatui::layout\ndone when: the dependency is gone and Layout::flex does the job\nEND",
    ),
    (
        110,
        "builder-2",
        7,
        "MSG builder-2 -> referee{re}\nkind: fixed\ntitle: the exact width is back and the card is 40 wide again\ncommit: 77d0a2c\nEND",
    ),
    (
        114,
        "builder",
        8,
        "MSG builder -> customs{re}\nkind: fixed\ntitle: ratatui-flexbox removed, Layout::flex used\ncommit: b4e9c11\nEND",
    ),
    (
        116,
        "referee",
        7,
        "RELEASE: 5e2f1b7\nMSG referee -> builder-2{re}\nkind: done\ntitle: ran cargo test card_width on a clean checkout, passes with the exact assert\nEND",
    ),
    (
        119,
        "customs",
        8,
        "RELEASE: 3c81f0e\nMSG customs -> builder{re}\nkind: done\ntitle: no invented packages left, every call exists in ratatui 0.30\nEND\nLESSON: look a package up on its registry before adding it; check the library you already have first",
    ),
    (
        126,
        "curator",
        6,
        "REPORT: MAP.md updated to b4e9c11: the new Flex layout and where the integrity check lives",
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
        app.burn = Some("-18%".into());
        app.task_started = Some(Instant::now() - Duration::from_secs(41 * 60 + 7));
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
                "ripple" => set(
                    a,
                    Status::Running,
                    "reach of 9e1b0d4: lr() has 11 callers",
                    "always on",
                    14_200,
                    &["3 leaf commits checked today, all clean"],
                ),
                "aegis" => set(
                    a,
                    Status::Running,
                    "reviewing guard.rs: input from agents",
                    "always on",
                    21_800,
                    &["2 commits today, nothing sensitive"],
                ),
                "bursar" => set(
                    a,
                    Status::Idle,
                    "next review with the scout in 1 day",
                    "rules: 4 active, 1 rolled back",
                    5_100,
                    &["last: S4 cut tester tokens 22%, success held"],
                ),
                "referee" => set(
                    a,
                    Status::Running,
                    "verifying #2: builder's fixed claim",
                    "always on",
                    9_800,
                    &["claims checked today: 6, one bounced"],
                ),
                "customs" => set(
                    a,
                    Status::Idle,
                    "no manifest changes since 23:10",
                    "",
                    2_400,
                    &[],
                ),
                "anchor" => set(
                    a,
                    Status::Idle,
                    "task 5: 7 of 9 criteria have evidence",
                    "",
                    4_700,
                    &[],
                ),
                "curator" => set(
                    a,
                    Status::Idle,
                    "MAP.md up to date at a3f9c21",
                    "",
                    1_200,
                    &[],
                ),
                "scout" => set(
                    a,
                    Status::Idle,
                    "last scan 2 days ago",
                    "next scan in 1 day",
                    6_400,
                    &["last scan: nothing worth switching"],
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
                    ahead: 2,
                },
                Worktree {
                    name: "wt/builder-2".into(),
                    files: 2,
                    added: 96,
                    removed: 4,
                    ahead: 1,
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
                on: "main".into(),
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
            limit_act: 0,
            last_frame: 0,
            mail_i: 0,
            mail_from: Duration::ZERO,
            threads: [None; 9],
            scanned: false,
            free_i: 0,
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
        let new_frame = app.frame != self.last_frame;
        self.last_frame = app.frame;
        for i in 0..app.agents.len() {
            if new_frame && app.agents[i].status.busy() && self.rand().is_multiple_of(3) {
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
        // the builder runs out of codex limits, carries on with opus, and goes back after the reset
        let secs = t.as_secs();
        if let Some(i) = app.agent_index("builder") {
            if self.limit_act == 0 && secs >= 30 {
                self.limit_act = 1;
                app.demo_limit_hit(i, crate::app::now_unix() + 40);
            } else if self.limit_act == 1 && secs >= 72 {
                self.limit_act = 2; // check_resets has run by now and asked to go back after this run
            } else if self.limit_act == 2 && secs >= 80 {
                self.limit_act = 3;
                app.agent_event(
                    i,
                    crate::vendors::AgentEvent::Done {
                        ok: true,
                        cost_usd: None,
                    },
                );
                app.agents[i].status = Status::Running;
                app.agents[i].now = "src/ui/team.rs".into();
                app.agents[i].task = "task 6 · wt/builder".into();
            }
        }
        // messages between agents, replayed every 100 seconds
        let at = t.saturating_sub(self.mail_from).as_secs();
        while let Some((when, from, thread, text)) = MAIL.get(self.mail_i).copied() {
            if at < when {
                break;
            }
            self.mail_i += 1;
            let re = self.threads[thread]
                .map(|id| format!(" re #{id}"))
                .unwrap_or_default();
            // through the same path real agent output takes: cards and protocol lines
            let first_new = app.messages.last().map_or(1, |m| m.id + 1);
            if let Some(i) = app.agent_index(from) {
                let keep = (app.agents[i].status, app.agents[i].now.clone());
                app.agent_event(
                    i,
                    crate::vendors::AgentEvent::Text(text.replace("{re}", &re)),
                );
                (app.agents[i].status, app.agents[i].now) = keep;
            }
            if app.messages.last().is_some_and(|m| m.id >= first_new) {
                self.threads[thread].get_or_insert(first_new);
            }
        }
        // the scanner catches a key in a commit: no model involved
        if !self.scanned && at >= 77 {
            self.scanned = true;
            let hit = |what, file: &str, line| crate::secrets::Finding {
                file: file.into(),
                line,
                what,
            };
            app.scanner_hit(
                "1d93f7e",
                &[
                    hit("Anthropic API key", "tests/fixtures/live.env", 3),
                    hit("email address", "tests/fixtures/live.env", 7),
                ],
            );
        }
        // the free checks and a fresh-context restart, once per pass
        if self.free_i == 0 && at >= 100 {
            self.free_i = 1;
            let f = |what, file: &str, line| crate::secrets::Finding {
                file: file.into(),
                line,
                what,
            };
            app.integrity_hit("5e2f1b7", &[f("an assert removed", "src/ui/team.rs", 0)]);
        }
        if self.free_i == 1 && at >= 106 {
            self.free_i = 2;
            let pkg = [("crates.io", "ratatui-flexbox".to_string())];
            app.deps_checked("3c81f0e", &pkg, &pkg);
        }
        if self.free_i == 2 && at >= 123 {
            self.free_i = 3;
            if let Some(i) = app.agent_index("builder") {
                app.refresh(i, "over 35 minutes");
                app.agents[i].now = "fresh start: reading the handoff".into();
            }
        }
        if self.mail_i == MAIL.len() && at >= 140 {
            self.mail_i = 0;
            self.mail_from = t;
            self.threads = [None; 9];
            self.scanned = false;
            self.free_i = 0;
        }
        // questions arrive while everyone keeps working
        let due = [2u64, 22, 45, 60];
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
                    change: None,
                    release: None,
                    budget: false,
                    asked: Instant::now(),
                },
                2 => Question {
                    from: "builder-2".into(),
                    task: "task 4".into(),
                    text: "Codex accent colour: blue or white?".into(),
                    options: vec!["blue".into(), "white".into()],
                    chosen: None,
                    guard_cmd: None,
                    change: None,
                    release: None,
                    budget: false,
                    asked: Instant::now(),
                },
                3 => {
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
                        change: None,
                        release: None,
                        budget: false,
                        asked: Instant::now(),
                    }
                }
                _ => {
                    if let Some(i) = app.agent_index("scout") {
                        app.agents[i].say("1 recommendation for you");
                    }
                    Question {
                        from: "scout".into(),
                        task: "models".into(),
                        text: "Move tester from claude sonnet to codex gpt-6-sol? demo data: in orda, tester runs on sonnet failed 3 of 11".into(),
                        options: vec!["switch to gpt-6-sol".into(), "keep it".into()],
                        chosen: None,
                        guard_cmd: None,
                        change: Some(Change { agent: "tester".into(), vendor: "codex".into(), model: "gpt-6-sol".into() }),
                        release: None,
                        budget: false,
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

#[cfg(test)]
mod tests {
    use crate::app::App;

    /// Jump the demo clock forward and play everything due by then.
    fn at(app: &mut App, secs: u64) {
        let mut demo = app.demo.take().unwrap();
        demo.start = std::time::Instant::now() - std::time::Duration::from_secs(secs);
        demo.step(app);
        app.demo = Some(demo);
    }

    #[test]
    fn ripple_holds_then_releases_and_teaches() {
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let mut app = App::new(true, tx);
        app.cfg.notify.desktop = false;
        at(&mut app, 41);
        assert!(
            app.holds
                .iter()
                .any(|h| h.0 == "9e1b0d4" && h.2 == "ripple")
        );
        let (bug_to, bug_ok, bug_thread) = app
            .messages
            .iter()
            .find(|m| m.from == "ripple" && m.card.kind == "bug")
            .map(|m| (m.to.clone(), m.card.missing().is_empty(), m.thread))
            .unwrap();
        assert_eq!(bug_to, "builder-2");
        assert!(bug_ok);
        at(&mut app, 56);
        assert!(app.holds.is_empty(), "released after the fix held");
        assert!(
            app.log.iter().any(
                |l| l.who == "ripple" && l.text.starts_with("lesson: when you change a helper")
            )
        );
        let fixed = app
            .messages
            .iter()
            .find(|m| m.from == "builder-2" && m.to == "ripple")
            .unwrap();
        assert_eq!(fixed.thread, bug_thread, "the fix answers ripple's thread");
    }

    /// Plays the whole script with no async runtime: starting a real agent would panic.
    #[test]
    fn demo_never_starts_a_real_agent() {
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let mut app = App::new(true, tx);
        app.cfg.notify.desktop = false;
        for s in 0..=145 {
            at(&mut app, s);
            for f in app.flights.iter_mut() {
                f.start -= std::time::Duration::from_secs(2);
            }
            app.tick();
        }
        assert!(app.messages.len() >= 10, "the script ran");
    }

    #[test]
    fn aegis_scanner_and_bursar_scenes() {
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let mut app = App::new(true, tx);
        app.cfg.notify.desktop = false;
        at(&mut app, 61);
        assert!(app.holds.iter().any(|h| h.0 == "4b08aa3" && h.2 == "aegis"));
        at(&mut app, 73);
        assert!(
            !app.holds.iter().any(|h| h.0 == "4b08aa3"),
            "released after the fix"
        );
        at(&mut app, 78);
        let q = app
            .questions
            .iter()
            .find(|q| q.from == "scanner")
            .expect("the owner is asked");
        assert_eq!(q.release.as_deref(), Some("1d93f7e"));
        assert!(
            app.holds
                .iter()
                .any(|h| h.0 == "1d93f7e" && h.2 == "scanner")
        );
        assert!(
            app.messages
                .iter()
                .any(|m| m.from == "scanner" && m.to == "aegis")
        );
        at(&mut app, 85);
        assert!(
            app.savings
                .iter()
                .any(|r| r.active && r.scope == "all" && r.text.starts_with("Search with grep"))
        );
        assert!(
            app.messages
                .iter()
                .any(|m| m.from == "bursar" && m.to == "scout")
        );
        // answering "a test value" releases the scanner's hold
        app.q_sel = app
            .questions
            .iter()
            .position(|q| q.from == "scanner")
            .unwrap();
        app.focus = crate::app::Focus::Questions;
        app.handle(crate::app::Msg::Key(
            ratatui::crossterm::event::KeyEvent::from(ratatui::crossterm::event::KeyCode::Char(
                '2',
            )),
        ));
        assert!(!app.holds.iter().any(|h| h.0 == "1d93f7e"));
    }

    #[test]
    fn referee_customs_and_curator_scenes() {
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let mut app = App::new(true, tx);
        app.cfg.notify.desktop = false;
        at(&mut app, 104);
        assert!(
            app.messages
                .iter()
                .any(|m| m.from == "integrity" && m.to == "referee")
        );
        assert!(
            app.holds
                .iter()
                .any(|h| h.0 == "5e2f1b7" && h.2 == "referee")
        );
        at(&mut app, 107);
        assert!(
            app.holds
                .iter()
                .any(|h| h.0 == "3c81f0e" && h.2 == "customs"),
            "an invented package is held at once"
        );
        assert!(
            app.messages
                .iter()
                .any(|m| m.from == "deps" && m.to == "customs")
        );
        at(&mut app, 120);
        assert!(
            !app.holds
                .iter()
                .any(|h| h.0 == "5e2f1b7" || h.0 == "3c81f0e"),
            "both released"
        );
        assert!(app.log.iter().any(|l| l.who == "referee" && l.text.starts_with("lesson: never loosen an assert")));
        at(&mut app, 124);
        assert!(
            app.log.iter().any(
                |l| l.who == "builder" && l.text.starts_with("fresh context (over 35 minutes)")
            )
        );
    }
}
