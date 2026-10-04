//! One adapter per vendor turns that CLI's JSON stream into [`AgentEvent`].
//! Nothing above this module knows any vendor's format.

mod claude;
mod codex;

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
pub fn spawn<M: Send + 'static>(
    vendor: &str,
    model: &str,
    effort: &str,
    prompt: &str,
    tx: UnboundedSender<M>,
    wrap: fn(usize, AgentEvent) -> M,
    slot: usize,
) {
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
            c
        }
        "codex" => {
            let mut c = Command::new("codex");
            c.args(["exec", "--json", "--skip-git-repo-check", "-m", model]);
            if !effort.is_empty() {
                c.args(["-c", &format!("model_reasoning_effort={effort}")]);
            }
            c.arg(prompt);
            c
        }
        other => {
            let _ = tx.send(wrap(
                slot,
                AgentEvent::Error(format!("no adapter for {other} yet")),
            ));
            return;
        }
    };
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let vendor = vendor.to_string();

    tokio::spawn(async move {
        let send = |e: AgentEvent| tx.send(wrap(slot, e)).is_ok();
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
                done |= matches!(ev, AgentEvent::Done { .. });
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
                send(AgentEvent::Error(short(last, 120)));
            }
        }
    });
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
mod live {
    /// Spends a few cents: `cargo test live -- --ignored`
    #[tokio::test]
    #[ignore]
    async fn claude_end_to_end() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        super::spawn(
            "claude",
            "haiku",
            "",
            "Reply with the single word: pong",
            tx,
            |_, e| e,
            0,
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
                |e| matches!(e, super::AgentEvent::Text(t) if t.to_lowercase().contains("pong"))
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
