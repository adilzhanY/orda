//! `claude -p --output-format stream-json --verbose`

use super::{AgentEvent, short, tool_detail};
use serde_json::Value;

pub fn parse(v: &Value) -> Vec<AgentEvent> {
    let s = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or("");
    match s("type") {
        "system" if s("subtype") == "init" => {
            vec![AgentEvent::Started {
                model: Some(s("model").to_string()),
            }]
        }
        // hooks, thinking token estimates and the like: nothing to show
        "system" => vec![],
        "assistant" => assistant(&v["message"]),
        "user" => tool_results(&v["message"]),
        "rate_limit_event" => {
            let info = &v["rate_limit_info"];
            let mut out = limits(info);
            // rejected and not covered by paid overage: this run cannot go on
            let overage = info["overageStatus"].as_str().unwrap_or("");
            if info["status"] == "rejected" && overage != "allowed" && overage != "allowed_warning"
            {
                out.push(AgentEvent::LimitHit {
                    resets_at: info["resetsAt"].as_i64(),
                    message: format!(
                        "claude {} limit reached",
                        info["rateLimitType"].as_str().unwrap_or("usage")
                    ),
                });
            }
            out
        }
        "result"
            if v["is_error"] == true
                && (v["api_error_status"] == 429 || super::is_limit(s("result"))) =>
        {
            vec![AgentEvent::LimitHit {
                resets_at: None,
                message: short(s("result"), 120),
            }]
        }
        "result" => {
            let u = &v["usage"];
            let n = |k: &str| u.get(k).and_then(Value::as_u64).unwrap_or(0);
            vec![
                AgentEvent::Tokens {
                    input: n("input_tokens")
                        + n("cache_creation_input_tokens")
                        + n("cache_read_input_tokens"),
                    output: n("output_tokens"),
                },
                AgentEvent::Done {
                    ok: s("subtype") == "success" && !v["is_error"].as_bool().unwrap_or(false),
                    cost_usd: v["total_cost_usd"].as_f64(),
                },
            ]
        }
        other => vec![AgentEvent::Unknown(format!("claude: {other}"))],
    }
}

fn assistant(msg: &Value) -> Vec<AgentEvent> {
    let mut out = vec![];
    if let Some(u) = msg.get("usage") {
        let n = |k: &str| u.get(k).and_then(Value::as_u64).unwrap_or(0);
        let ctx =
            n("input_tokens") + n("cache_creation_input_tokens") + n("cache_read_input_tokens");
        if ctx > 0 {
            out.push(AgentEvent::Context(ctx));
        }
    }
    for c in msg["content"].as_array().into_iter().flatten() {
        match c["type"].as_str().unwrap_or("") {
            "text" => {
                let t = c["text"].as_str().unwrap_or("").trim();
                if !t.is_empty() {
                    out.push(AgentEvent::Text(t.to_string()));
                }
            }
            "tool_use" => {
                let tool = c["name"].as_str().unwrap_or("tool").to_string();
                let detail = tool_detail(&c["input"]);
                if tool == "WebSearch" || tool == "WebFetch" {
                    out.push(AgentEvent::WebSearch {
                        query: detail.clone(),
                    });
                }
                out.push(AgentEvent::ToolCall { tool, detail });
            }
            "thinking" | "redacted_thinking" => {}
            other => out.push(AgentEvent::Unknown(format!("claude content: {other}"))),
        }
    }
    out
}

fn tool_results(msg: &Value) -> Vec<AgentEvent> {
    msg["content"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|c| c["type"] == "tool_result")
        .map(|c| {
            let text = match &c["content"] {
                Value::String(s) => s.clone(),
                Value::Array(parts) => parts
                    .iter()
                    .filter_map(|p| p["text"].as_str())
                    .collect::<Vec<_>>()
                    .join(" "),
                _ => String::new(),
            };
            let first = text.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
            AgentEvent::ToolResult {
                ok: !c["is_error"].as_bool().unwrap_or(false),
                detail: short(first, 80),
            }
        })
        .collect()
}

fn limits(info: &Value) -> Vec<AgentEvent> {
    info["unifiedWindows"]
        .as_object()
        .into_iter()
        .flatten()
        .map(|(window, w)| AgentEvent::Limit {
            window: window.clone(),
            used_pct: w["utilization"].as_f64().unwrap_or(0.0) * 100.0,
            resets_at: w["resetsAt"].as_i64().unwrap_or(0),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::{AgentEvent, parse};

    #[test]
    fn real_stream() {
        let events: Vec<_> = include_str!("../../tests/fixtures/claude.jsonl")
            .lines()
            .flat_map(|l| parse("claude", l))
            .collect();
        assert!(
            matches!(&events[0], AgentEvent::Started { model: Some(m) } if m.contains("haiku"))
        );
        assert!(events.iter().any(|e| matches!(e, AgentEvent::ToolCall { tool, detail } if tool == "Bash" && detail == "ls -la")));
        assert!(
            events
                .iter()
                .any(|e| matches!(e, AgentEvent::ToolResult { ok: true, .. }))
        );
        assert!(
            events
                .iter()
                .any(|e| matches!(e, AgentEvent::Limit { window, .. } if window == "five_hour"))
        );
        assert!(events.iter().any(|e| e == &AgentEvent::Text("done".into())));
        assert!(matches!(
            events.last(),
            Some(AgentEvent::Done {
                ok: true,
                cost_usd: Some(_)
            })
        ));
        assert!(
            !events.iter().any(|e| matches!(e, AgentEvent::Unknown(_))),
            "{events:?}"
        );
    }

    #[test]
    fn rejected_limit_is_a_limit_hit() {
        let line = r#"{"type":"rate_limit_event","rate_limit_info":{"status":"rejected","resetsAt":1791161400,"rateLimitType":"five_hour","overageStatus":"rejected"}}"#;
        let e = parse("claude", line);
        assert!(
            e.contains(&AgentEvent::LimitHit {
                resets_at: Some(1791161400),
                message: "claude five_hour limit reached".into()
            }),
            "{e:?}"
        );
        let warn = r#"{"type":"rate_limit_event","rate_limit_info":{"status":"allowed_warning","resetsAt":1}}"#;
        assert!(
            !parse("claude", warn)
                .iter()
                .any(|e| matches!(e, AgentEvent::LimitHit { .. }))
        );
    }

    #[test]
    fn unknown_type_is_reported() {
        assert_eq!(
            parse("claude", r#"{"type":"brand_new"}"#),
            vec![AgentEvent::Unknown("claude: brand_new".into())]
        );
    }
}
