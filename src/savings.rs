//! The bursar's savings rules, kept as experiments: each rule records the success
//! rate and token use of its scope when it was added, and orda rolls it back when
//! the success rate falls after it. ~/.local/share/orda/savings.json

use crate::stats::Run;
use serde::{Deserialize, Serialize};

/// A rule is judged after this many runs in its scope.
pub const MIN_RUNS: usize = 10;
/// Rolled back when the success rate drops by more than this (0.10 = 10 points).
pub const MAX_DROP: f64 = 0.10;
/// The bursar keeps at most this many rules active.
pub const MAX_ACTIVE: usize = 12;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Rule {
    pub id: String,
    /// "all" or a role name
    pub scope: String,
    pub text: String,
    pub added: i64,
    pub base_ok: f64,
    pub base_tokens: f64,
    pub active: bool,
    #[serde(default)]
    pub dropped: String,
}

fn path() -> std::path::PathBuf {
    crate::stats::dir().join("savings.json")
}

pub fn load() -> Vec<Rule> {
    std::fs::read_to_string(path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn store(rules: &[Rule]) {
    let _ = std::fs::create_dir_all(crate::stats::dir());
    if let Ok(s) = serde_json::to_string_pretty(rules) {
        let _ = std::fs::write(path(), s);
    }
}

fn in_scope<'a>(runs: &'a [Run], scope: &'a str) -> impl Iterator<Item = &'a Run> {
    runs.iter()
        .filter(move |r| scope == "all" || r.role == scope)
}

/// Success rate and average tokens of a set of runs.
fn measure<'a>(runs: impl Iterator<Item = &'a Run>) -> Option<(f64, f64, usize)> {
    let rs: Vec<&Run> = runs.collect();
    if rs.is_empty() {
        return None;
    }
    let n = rs.len() as f64;
    let ok = rs.iter().filter(|r| r.ok).count() as f64 / n;
    let tokens = rs.iter().map(|r| r.tokens as f64).sum::<f64>() / n;
    Some((ok, tokens, rs.len()))
}

/// Add a rule with the current numbers of its scope as the baseline. Returns its id.
pub fn add(
    rules: &mut Vec<Rule>,
    scope: &str,
    text: &str,
    runs: &[Run],
    now: i64,
) -> Option<String> {
    if rules.iter().any(|r| r.active && r.text == text) {
        return None;
    }
    let n = rules.len() + 1;
    let (base_ok, base_tokens, _) = measure(in_scope(runs, scope)).unwrap_or((1.0, 0.0, 0));
    let id = format!("S{n}");
    rules.push(Rule {
        id: id.clone(),
        scope: scope.into(),
        text: text.into(),
        added: now,
        base_ok,
        base_tokens,
        active: true,
        dropped: String::new(),
    });
    Some(id)
}

pub fn drop(rules: &mut [Rule], id: &str, why: &str) -> bool {
    match rules.iter_mut().find(|r| r.id == id && r.active) {
        Some(r) => {
            r.active = false;
            r.dropped = why.into();
            true
        }
        None => false,
    }
}

/// Roll back every rule whose scope got less successful since it was added.
/// Returns (id, explanation) for each rule it rolled back.
pub fn judge(rules: &mut [Rule], runs: &[Run]) -> Vec<(String, String)> {
    let mut out = vec![];
    for r in rules.iter_mut().filter(|r| r.active) {
        let after = in_scope(runs, &r.scope).filter(|x| x.at >= r.added);
        if let Some((ok, _, n)) = measure(after)
            && n >= MIN_RUNS
            && ok < r.base_ok - MAX_DROP
        {
            r.active = false;
            r.dropped = format!(
                "success fell from {:.0}% to {:.0}% over {n} runs",
                r.base_ok * 100.0,
                ok * 100.0
            );
            out.push((r.id.clone(), r.dropped.clone()));
        }
    }
    out
}

/// The rules an agent of `role` follows, as a markdown list.
pub fn for_role(rules: &[Rule], role: &str) -> String {
    rules
        .iter()
        .filter(|r| r.active && (r.scope == "all" || r.scope == role))
        .map(|r| format!("- {} ({})\n", r.text, r.id))
        .collect()
}

/// Every rule with its measured effect so far, for the bursar's task.
pub fn report(rules: &[Rule], runs: &[Run]) -> String {
    if rules.is_empty() {
        return "No rules yet.\n".into();
    }
    let mut s = String::from(
        "| Id | Scope | Rule | Active | Runs since | Tokens per run, before -> after | Success, before -> after |\n|---|---|---|---|---|---|---|\n",
    );
    for r in rules {
        let after = measure(in_scope(runs, &r.scope).filter(|x| x.at >= r.added));
        let (n, tok, ok) = match after {
            Some((ok, tok, n)) => (
                n,
                format!("{:.0} -> {tok:.0}", r.base_tokens),
                format!("{:.0}% -> {:.0}%", r.base_ok * 100.0, ok * 100.0),
            ),
            None => (
                0,
                format!("{:.0} -> ?", r.base_tokens),
                format!("{:.0}% -> ?", r.base_ok * 100.0),
            ),
        };
        let active = if r.active {
            "yes".to_string()
        } else {
            format!("no: {}", r.dropped)
        };
        s += &format!(
            "| {} | {} | {} | {active} | {n} | {tok} | {ok} |\n",
            r.id, r.scope, r.text
        );
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(at: i64, role: &str, ok: bool, tokens: u64) -> Run {
        Run {
            at,
            role: role.into(),
            ok,
            tokens,
            ..Default::default()
        }
    }

    #[test]
    fn a_rule_that_hurts_is_rolled_back() {
        // the tester succeeded 9 of 10 times before the rule
        let mut runs: Vec<Run> = (0..10).map(|i| run(i, "tester", i != 0, 1000)).collect();
        let mut rules = vec![];
        let id = add(
            &mut rules,
            "tester",
            "read only the failing test output",
            &runs,
            100,
        )
        .unwrap();
        assert_eq!(rules[0].base_ok, 0.9);
        assert!(
            add(
                &mut rules,
                "tester",
                "read only the failing test output",
                &runs,
                100
            )
            .is_none(),
            "no duplicates"
        );
        // other roles do not count against it
        runs.extend((0..10).map(|i| run(200 + i, "builder", false, 500)));
        assert!(judge(&mut rules, &runs).is_empty());
        // fewer tokens but only 6 of 10 succeed: rolled back
        runs.extend((0..10).map(|i| run(300 + i, "tester", i < 6, 400)));
        let gone = judge(&mut rules, &runs);
        assert_eq!(gone[0].0, id);
        assert!(!rules[0].active && rules[0].dropped.contains("90% to 60%"));
        assert!(for_role(&rules, "tester").is_empty());
    }

    #[test]
    fn a_rule_that_saves_stays() {
        let mut runs: Vec<Run> = (0..10).map(|i| run(i, "builder", true, 1000)).collect();
        let mut rules = vec![];
        add(&mut rules, "all", "grep before opening a file", &runs, 100);
        runs.extend((0..10).map(|i| run(200 + i, "builder", true, 700)));
        assert!(judge(&mut rules, &runs).is_empty());
        assert_eq!(
            for_role(&rules, "builder"),
            "- grep before opening a file (S1)\n"
        );
        assert!(report(&rules, &runs).contains("1000 -> 700"));
    }
}
