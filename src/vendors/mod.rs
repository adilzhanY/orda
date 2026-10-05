//! One adapter per vendor turns that CLI's JSON stream into [`AgentEvent`].
//! Nothing above this module knows any vendor's format.

mod claude;
mod codex;

use crate::config::AgentDef;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug, Clone, PartialEq)]
pub enum AgentEvent {
    Started {
        model: Option<String>,
    },
    Text(String),
    ToolCall {
        tool: String,
        detail: String,
    },
    ToolResult {
        ok: bool,
        detail: String,
    },
    WebSearch {
        query: String,
    },
    /// Size of the context the model just read.
    Context(u64),
    /// Tokens to add to the agent's totals.
    Tokens {
        input: u64,
        output: u64,
    },
    /// A vendor rate limit window, as reported mid-stream.
    Limit {
        window: String,
        used_pct: f64,
        resets_at: i64,
    },
    Done {
        ok: bool,
        cost_usd: Option<f64>,
    },
    /// The vendor refused because a plan limit ran out. `resets_at` is unix seconds when known.
    LimitHit {
        resets_at: Option<i64>,
        message: String,
    },
    Error(String),
    /// An event this adapter does not understand. Shown, never dropped silently.
    Unknown(String),
}

/// Parse one line of a vendor's JSON stream.
pub fn parse(vendor: &str, line: &str) -> Vec<AgentEvent> {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
        return vec![AgentEvent::Unknown(format!(
            "not json: {}",
            short(line, 60)
        ))];
    };
    match vendor {
        "codex" => codex::parse(&v),
        _ => claude::parse(&v),
    }
}

/// Run one agent headless and send its events, tagged with `slot`, until it exits.
/// `system` is the agent's standing instructions (TEAM.md, its AGENT.md, the roster).
/// Returns a handle that stops the run (and kills the process) when aborted.
pub fn spawn<M: Send + 'static>(
    def: &AgentDef,
    system: &str,
    prompt: &str,
    tx: UnboundedSender<M>,
    wrap: impl Fn(AgentEvent) -> M + Send + Sync + 'static,
) -> Option<tokio::task::AbortHandle> {
    let (vendor, model, effort) = (def.vendor.as_str(), def.model.as_str(), def.effort.as_str());
    let mut cmd = match vendor {
        "claude" => {
            let mut c = Command::new("claude");
            c.args([
                "-p",
                prompt,
                "--output-format",
                "stream-json",
                "--verbose",
                "--model",
                model,
            ]);
            if !effort.is_empty() {
                c.args(["--effort", effort]);
            }
            if !system.is_empty() {
                c.args(["--append-system-prompt", system]);
            }
            // every agent may search the web (TEAM.md asks them to)
            c.args(["--allowedTools", "WebSearch,WebFetch"]);
            c
        }
        "codex" => {
            let mut c = Command::new("codex");
            c.args(["exec", "--json", "--skip-git-repo-check", "-m", model]);
            if !effort.is_empty() {
                c.args(["-c", &format!("model_reasoning_effort={effort}")]);
            }
            // codex exec has no system prompt flag: the instructions go in front of the task
            c.arg(if system.is_empty() {
                prompt.to_string()
            } else {
                format!("{system}\n---\n\n# Your task\n\n{prompt}")
            });
            c
        }
        other => {
            let _ = tx.send(wrap(AgentEvent::Error(format!(
                "no adapter for {other} yet"
            ))));
            return None;
        }
    };
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let vendor = vendor.to_string();

    let task = tokio::spawn(async move {
        let send = |e: AgentEvent| tx.send(wrap(e)).is_ok();
        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                send(AgentEvent::Error(format!("could not start {vendor}: {e}")));
                return;
            }
        };
        let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
        let mut done = false;
        while let Ok(Some(line)) = lines.next_line().await {
            for ev in parse(&vendor, &line) {
                done |= matches!(ev, AgentEvent::Done { .. } | AgentEvent::LimitHit { .. });
                if !send(ev) {
                    return;
                }
            }
        }
        let mut err = String::new();
        if let Some(mut s) = child.stderr.take() {
            let _ = s.read_to_string(&mut err).await;
        }
        let status = child.wait().await;
        if !done {
            let ok = status.map(|s| s.success()).unwrap_or(false);
            if ok {
                send(AgentEvent::Done { ok, cost_usd: None });
            } else {
                let last = err
                    .lines()
                    .rev()
                    .find(|l| !l.trim().is_empty())
                    .unwrap_or("exited with an error");
                send(if is_limit(last) {
                    AgentEvent::LimitHit {
                        resets_at: try_again_at(last, now_unix()),
                        message: short(last, 120),
                    }
                } else {
                    AgentEvent::Error(short(last, 120))
                });
            }
        }
    });
    Some(task.abort_handle())
}

/// True for the messages vendors print when a plan limit ran out.
pub fn is_limit(msg: &str) -> bool {
    let m = msg.to_lowercase();
    [
        "usage limit",
        "rate limit",
        "limit reached",
        "hit your limit",
        "out of extra usage",
        "quota exceeded",
        "resource_exhausted",
    ]
    .iter()
    .any(|k| m.contains(k))
}

/// The reset time in a message like "... try again at 3:05 PM." as unix seconds:
/// the next time that wall-clock time comes round.
pub fn try_again_at(msg: &str, now: i64) -> Option<i64> {
    let lower = msg.to_lowercase();
    let rest = &lower[lower.find("try again at")? + 12..];
    let colon = rest.find(':')?;
    let hour: i64 = rest[..colon]
        .rsplit(|c: char| !c.is_ascii_digit())
        .next()?
        .parse()
        .ok()?;
    let minute: i64 = rest.get(colon + 1..colon + 3)?.parse().ok()?;
    let after = &rest[colon + 3..];
    let pm = after.trim_start().starts_with("pm");
    let am = after.trim_start().starts_with("am");
    let hour = match (am, pm, hour) {
        (_, true, h) if h < 12 => h + 12,
        (true, _, 12) => 0,
        (_, _, h) => h,
    };
    let off = crate::app::utc_offset();
    let local_now = now + off;
    let day = local_now - local_now.rem_euclid(86400);
    let mut at = day + hour * 3600 + minute * 60 - off;
    if at <= now {
        at += 86400;
    }
    Some(at)
}

fn now_unix() -> i64 {
    crate::app::now_unix()
}

pub fn short(s: &str, max: usize) -> String {
    let one = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if one.chars().count() <= max {
        return one;
    }
    let mut t: String = one.chars().take(max.saturating_sub(1)).collect();
    t.push('…');
    t
}

/// A readable one-liner for a tool input: the command, path, query or URL it acts on.
fn tool_detail(input: &serde_json::Value) -> String {
    for key in [
        "command",
        "file_path",
        "path",
        "pattern",
        "query",
        "url",
        "description",
    ] {
        if let Some(s) = input.get(key).and_then(|v| v.as_str()) {
            return short(s, 80);
        }
    }
    String::new()
}

#[cfg(test)]
mod limit_tests {
    use super::*;

    #[test]
    fn limit_messages() {
        assert!(is_limit("You've hit your usage limit. Upgrade to Pro"));
        assert!(is_limit("You've hit your limit · resets 5pm"));
        assert!(is_limit("Claude AI usage limit reached|1791161400"));
        assert!(!is_limit("error: cargo test failed"));
    }

    #[test]
    fn try_again_at_is_the_next_such_time() {
        let off = crate::app::utc_offset();
        // local midnight of some day, plus 10:00
        let day = 1_791_000_000 - (1_791_000_000 + off).rem_euclid(86400);
        let now = day + 10 * 3600;
        let at = |m: &str| try_again_at(m, now).map(|t| t - day);
        assert_eq!(at("or try again at 3:05 PM."), Some(15 * 3600 + 5 * 60));
        assert_eq!(at("try again at 11:30 AM"), Some(11 * 3600 + 30 * 60));
        assert_eq!(
            at("try again at 9:00 AM"),
            Some(86400 + 9 * 3600),
            "earlier today means tomorrow"
        );
        assert_eq!(at("try again at 12:15 AM"), Some(86400 + 15 * 60));
        assert_eq!(at("try again later"), None);
    }
}

#[cfg(test)]
mod live {
    /// Spends a few cents: `cargo test live -- --ignored`
    #[tokio::test]
    #[ignore]
    async fn claude_end_to_end() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let cfg = crate::config::Config::default();
        let mut me = cfg
            .agents
            .iter()
            .find(|a| a.role == "tester")
            .unwrap()
            .clone();
        me.model = "haiku".into();
        me.effort.clear();
        let system = crate::roles::instructions(&me, &cfg.agents, &[], "", "");
        super::spawn(
            &me,
            &system,
            "Do not use any tools. Reply with exactly one REPORT line, as your instructions describe, saying: pong",
            tx,
            |e| e,
        );
        let mut events = vec![];
        while let Some(e) = rx.recv().await {
            let done = matches!(
                e,
                super::AgentEvent::Done { .. } | super::AgentEvent::Error(_)
            );
            events.push(e);
            if done {
                break;
            }
        }
        assert!(
            events.iter().any(
                |e| matches!(e, super::AgentEvent::Text(t) if t.lines().any(|l| matches!(crate::roles::protocol(l), Some(crate::roles::Line::Report(r)) if r.to_lowercase().contains("pong"))))
            ),
            "{events:?}"
        );
        assert!(
            matches!(
                events.last(),
                Some(super::AgentEvent::Done { ok: true, .. })
            ),
            "{events:?}"
        );
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, super::AgentEvent::Unknown(_))),
            "{events:?}"
        );
    }
}

#[cfg(test)]
mod live_card {
    /// Does a real model write a well-formed card? `cargo test live_card -- --ignored --nocapture`
    #[tokio::test]
    #[ignore]
    async fn tester_sends_a_bug_card() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let cfg = crate::config::Config::default();
        let mut me = cfg
            .agents
            .iter()
            .find(|a| a.role == "tester")
            .unwrap()
            .clone();
        me.model = "haiku".into();
        me.effort.clear();
        let system = crate::roles::instructions(&me, &cfg.agents, &[], "", "");
        let task = "Do not use any tools. Pretend you just tested commit a3f9c21 on branch wt/builder and found this: \
                    `cargo test codex::unknown_item` panics at src/vendors/codex.rs:48 when codex sends a turn.diff event, \
                    where it should return AgentEvent::Unknown. Tell the right agent.";
        super::spawn(&me, &system, task, tx, |e| e);
        let mut text = String::new();
        while let Some(e) = rx.recv().await {
            match e {
                super::AgentEvent::Text(t) => text += &format!("{t}\n"),
                super::AgentEvent::Done { .. } | super::AgentEvent::Error(_) => break,
                _ => {}
            }
        }
        let (cards, _) = crate::roles::cards(&text);
        println!("{text}");
        let card = cards.first().expect("the tester sent a card");
        assert_eq!((card.to.as_str(), card.kind.as_str()), ("builder", "bug"));
        assert!(card.missing().is_empty(), "missing {:?}", card.missing());
    }
}

#[cfg(test)]
mod live_scout {
    /// A real scout scan with web search: `cargo test live_scout -- --ignored --nocapture`
    #[tokio::test]
    #[ignore]
    async fn scout_scan() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let cfg = crate::config::Config::default();
        let me = cfg
            .agents
            .iter()
            .find(|a| a.role == "scout")
            .unwrap()
            .clone();
        let system = crate::roles::instructions(&me, &cfg.agents, &[], "", "");
        let task = format!(
            "Run your scan. Today is {}. This is your first scan.\n\n## orda's own record\n\n{}",
            "2026-10-05",
            crate::stats::summary()
        );
        super::spawn(&me, &system, &task, tx, |e| e);
        let mut searched = false;
        while let Some(e) = rx.recv().await {
            match &e {
                super::AgentEvent::WebSearch { query } => {
                    searched = true;
                    println!("SEARCH {query}");
                }
                super::AgentEvent::Text(t) => {
                    for l in t.lines() {
                        if let Some(p) = crate::roles::protocol(l) {
                            println!("{p:?}");
                        }
                    }
                }
                super::AgentEvent::Done { .. } | super::AgentEvent::Error(_) => {
                    println!("{e:?}");
                    break;
                }
                _ => {}
            }
        }
        assert!(searched, "the scout never searched the web");
    }
}
