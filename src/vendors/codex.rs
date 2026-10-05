//! `codex exec --json`

use super::{AgentEvent, short};
use serde_json::Value;

pub fn parse(v: &Value) -> Vec<AgentEvent> {
    let item = &v["item"];
    let kind = item["type"].as_str().unwrap_or("");
    match v["type"].as_str().unwrap_or("") {
        "thread.started" => vec![AgentEvent::Started { model: None }],
        "turn.started" => vec![],
        "turn.completed" => {
            let u = &v["usage"];
            let n = |k: &str| u.get(k).and_then(Value::as_u64).unwrap_or(0);
            vec![
                AgentEvent::Context(n("input_tokens")),
                // cached input costs little and is re-read every turn: it is not counted as spent
                AgentEvent::Tokens {
                    input: n("input_tokens").saturating_sub(n("cached_input_tokens")),
                    output: n("output_tokens"),
                },
                AgentEvent::Done {
                    ok: true,
                    cost_usd: None,
                },
            ]
        }
        // codex sends "error" for retries and again as "turn.failed" when it gives up: count it once
        "error" => vec![],
        "turn.failed" => {
            let msg = v["error"]["message"]
                .as_str()
                .or(v["message"].as_str())
                .unwrap_or("codex failed");
            // the message is often JSON from the API; its inner message is the readable part
            let inner = serde_json::from_str::<Value>(msg)
                .ok()
                .and_then(|j| j["error"]["message"].as_str().map(String::from));
            let msg = inner.as_deref().unwrap_or(msg);
            if super::model_unavailable(msg) {
                vec![AgentEvent::ModelUnavailable(short(msg, 160))]
            } else if super::is_limit(msg) {
                vec![AgentEvent::LimitHit {
                    resets_at: super::try_again_at(msg, crate::app::now_unix()),
                    message: short(msg, 120),
                }]
            } else {
                vec![AgentEvent::Error(short(msg, 120))]
            }
        }
        "item.started" | "item.updated" => match kind {
            "command_execution" => vec![AgentEvent::ToolCall {
                tool: "shell".into(),
                detail: command(item),
            }],
            "web_search" => vec![AgentEvent::WebSearch { query: query(item) }],
            _ => vec![],
        },
        "item.completed" => match kind {
            "agent_message" => vec![AgentEvent::Text(
                item["text"].as_str().unwrap_or("").trim().to_string(),
            )],
            "command_execution" => {
                let out = item["aggregated_output"].as_str().unwrap_or("");
                let first = out.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
                vec![AgentEvent::ToolResult {
                    ok: item["exit_code"].as_i64() == Some(0),
                    detail: short(first, 80),
                }]
            }
            "file_change" => {
                let paths: Vec<_> = item["changes"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|c| c["path"].as_str())
                    .collect();
                vec![AgentEvent::ToolCall {
                    tool: "edit".into(),
                    detail: short(&paths.join(", "), 80),
                }]
            }
            "web_search" => vec![AgentEvent::WebSearch { query: query(item) }],
            "reasoning" | "todo_list" | "mcp_tool_call" => vec![],
            "error" => vec![AgentEvent::Text(format!(
                "codex: {}",
                item["message"].as_str().unwrap_or("")
            ))],
            other => vec![AgentEvent::Unknown(format!("codex item: {other}"))],
        },
        other => vec![AgentEvent::Unknown(format!("codex: {other}"))],
    }
}

fn command(item: &Value) -> String {
    let c = item["command"].as_str().unwrap_or("");
    short(c.strip_prefix("/usr/bin/bash -lc ").unwrap_or(c), 80)
}

fn query(item: &Value) -> String {
    short(item["query"].as_str().unwrap_or(""), 80)
}

#[cfg(test)]
mod tests {
    use super::super::{AgentEvent, parse};

    #[test]
    fn real_stream() {
        let events: Vec<_> = include_str!("../../tests/fixtures/codex.jsonl")
            .lines()
            .flat_map(|l| parse("codex", l))
            .collect();
        assert_eq!(events[0], AgentEvent::Started { model: None });
        assert!(events.contains(&AgentEvent::ToolCall {
            tool: "shell".into(),
            detail: "ls".into()
        }));
        assert!(
            events
                .iter()
                .any(|e| matches!(e, AgentEvent::ToolResult { ok: true, .. }))
        );
        assert!(events.contains(&AgentEvent::Text("done".into())));
        assert!(events.contains(&AgentEvent::Tokens {
            input: 39860 - 31488,
            output: 68
        }));
        assert!(matches!(
            events.last(),
            Some(AgentEvent::Done { ok: true, .. })
        ));
        assert!(
            !events.iter().any(|e| matches!(e, AgentEvent::Unknown(_))),
            "{events:?}"
        );
    }

    #[test]
    fn usage_limit_is_a_limit_hit() {
        let e = parse(
            "codex",
            r#"{"type":"turn.failed","error":{"message":"You've hit your usage limit. Upgrade to Pro (https://chatgpt.com/explore/pro), visit https://chatgpt.com/codex/settings/usage to purchase more credits or try again at 3:05 PM."}}"#,
        );
        assert!(
            matches!(
                &e[0],
                AgentEvent::LimitHit {
                    resets_at: Some(_),
                    ..
                }
            ),
            "{e:?}"
        );
    }

    #[test]
    fn a_model_the_plan_lacks_is_unavailable() {
        // what codex really printed for gpt-6.1-sol on a ChatGPT Plus account
        let line = r#"{"type":"turn.failed","error":{"message":"{\"type\":\"error\",\"status\":400,\"error\":{\"type\":\"invalid_request_error\",\"message\":\"The 'gpt-6.1-sol' model is not supported when using Codex with a ChatGPT account.\"}}"}}"#;
        assert_eq!(
            parse("codex", line),
            vec![AgentEvent::ModelUnavailable(
                "The 'gpt-6.1-sol' model is not supported when using Codex with a ChatGPT account."
                    .into()
            )]
        );
        assert!(
            parse(
                "codex",
                r#"{"type":"error","message":"Reconnecting... 1/5"}"#
            )
            .is_empty()
        );
    }

    #[test]
    fn failure_is_an_error() {
        let e = parse(
            "codex",
            r#"{"type":"turn.failed","error":{"message":"quota"}}"#,
        );
        assert_eq!(e, vec![AgentEvent::Error("quota".into())]);
    }
}
