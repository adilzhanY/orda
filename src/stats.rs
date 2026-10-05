//! orda's own record of how each model did, one JSON line per finished run,
//! in ~/.local/share/orda/stats.jsonl. The scout and the bursar read it.

use crate::config::{AgentDef, home};
use serde::Deserialize;
use serde_json::json;
use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;

pub fn dir() -> PathBuf {
    home().join(".local/share/orda")
}

#[derive(Deserialize, Clone, Default, Debug)]
#[serde(default)]
pub struct Run {
    pub at: i64,
    pub agent: String,
    pub role: String,
    pub vendor: String,
    pub model: String,
    pub ok: bool,
    pub tokens: u64,
    pub input: u64,
    pub output: u64,
    pub tools: u64,
    pub secs: u64,
}

pub fn record(def: &AgentDef, r: &Run) {
    let _ = std::fs::create_dir_all(dir());
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir().join("stats.jsonl"))
    {
        let line = json!({
            "at": r.at, "agent": def.name, "role": def.role, "vendor": def.vendor, "model": def.model,
            "ok": r.ok, "tokens": r.input + r.output, "input": r.input, "output": r.output,
            "tools": r.tools, "secs": r.secs,
        });
        let _ = writeln!(f, "{line}");
    }
}

pub fn runs() -> Vec<Run> {
    parse(&std::fs::read_to_string(dir().join("stats.jsonl")).unwrap_or_default())
}

fn parse(text: &str) -> Vec<Run> {
    text.lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

/// A markdown table per role and model: runs, successes, average tokens (in and out), tool calls and time.
pub fn summary() -> String {
    summarize(&runs())
}

fn summarize(runs: &[Run]) -> String {
    type Key = (String, String, String); // role, vendor, model
    let mut rows: BTreeMap<Key, Vec<&Run>> = BTreeMap::new();
    for r in runs {
        rows.entry((r.role.clone(), r.vendor.clone(), r.model.clone()))
            .or_default()
            .push(r);
    }
    if rows.is_empty() {
        return "No runs recorded yet.\n".into();
    }
    let mut out = String::from(
        "| Role | Vendor | Model | Runs | Succeeded | Avg tokens | Avg in | Avg out | Avg tool calls | Avg time |\n|---|---|---|---|---|---|---|---|---|---|\n",
    );
    for ((role, vendor, model), rs) in rows {
        let n = rs.len() as u64;
        let sum = |f: fn(&Run) -> u64| rs.iter().map(|r| f(r)).sum::<u64>() / n;
        let ok = rs.iter().filter(|r| r.ok).count();
        out += &format!(
            "| {role} | {vendor} | {model} | {n} | {ok} | {} | {} | {} | {} | {}s |\n",
            sum(|r| r.tokens),
            sum(|r| r.input),
            sum(|r| r.output),
            sum(|r| r.tools),
            sum(|r| r.secs)
        );
    }
    out
}

/// Average tokens per run over the last 7 days against the 7 before, as "-12%".
pub fn trend(runs: &[Run], now: i64) -> Option<String> {
    let week = 7 * 86400;
    let avg = |from: i64, to: i64| {
        let r: Vec<u64> = runs
            .iter()
            .filter(|r| r.at >= from && r.at < to)
            .map(|r| r.tokens)
            .collect();
        (!r.is_empty()).then(|| r.iter().sum::<u64>() as f64 / r.len() as f64)
    };
    let (this, last) = (avg(now - week, now + 1)?, avg(now - 2 * week, now - week)?);
    Some(format!("{:+.0}%", (this / last - 1.0) * 100.0))
}

/// When a periodic job (the scout's scan, aegis's audit) last finished.
pub fn last_run(job: &str) -> Option<i64> {
    std::fs::read_to_string(dir().join(format!("{job}-last-run")))
        .ok()?
        .trim()
        .parse()
        .ok()
}

pub fn mark_run(job: &str, at: i64) {
    let _ = std::fs::create_dir_all(dir());
    let _ = std::fs::write(dir().join(format!("{job}-last-run")), at.to_string());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_groups_by_role_and_model() {
        let text = r#"{"role":"tester","vendor":"claude","model":"sonnet","ok":true,"tokens":100,"input":90,"output":10,"tools":4,"secs":10}
{"role":"tester","vendor":"claude","model":"sonnet","ok":false,"tokens":300,"input":280,"output":20,"tools":8,"secs":30}
{"role":"builder","vendor":"codex","model":"gpt-6.1-sol","ok":true,"tokens":50,"secs":5}"#;
        let s = summarize(&parse(text));
        assert!(
            s.contains("| tester | claude | sonnet | 2 | 1 | 200 | 185 | 15 | 6 | 20s |"),
            "{s}"
        );
        assert!(
            s.contains("| builder | codex | gpt-6.1-sol | 1 | 1 | 50 | 0 | 0 | 0 | 5s |"),
            "older lines without the new fields still count"
        );
        assert_eq!(summarize(&[]), "No runs recorded yet.\n");
    }

    #[test]
    fn trend_compares_weeks() {
        let run = |at, tokens| Run {
            at,
            tokens,
            ..Default::default()
        };
        let now = 100 * 86400;
        let runs = [
            run(now - 86400, 80),
            run(now - 2 * 86400, 100),
            run(now - 9 * 86400, 100),
        ];
        assert_eq!(trend(&runs, now).as_deref(), Some("-10%"));
        assert_eq!(trend(&runs[..2], now), None);
    }
}
