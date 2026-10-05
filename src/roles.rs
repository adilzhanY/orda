//! The team's instructions (`agents/TEAM.md` and `agents/<role>/AGENT.md`) and the
//! line protocol agents use to talk to orda (`ASK:`, `REPORT:`, `LEARNED:`).
//!
//! The files are compiled in. A copy in `~/.config/orda/agents/` overrides one.

use crate::config::{self, AgentDef};

const TEAM: &str = include_str!("../agents/TEAM.md");

const BUILTIN: &[(&str, &str)] = &[
    ("boss", include_str!("../agents/boss/AGENT.md")),
    ("advisor", include_str!("../agents/advisor/AGENT.md")),
    ("builder", include_str!("../agents/builder/AGENT.md")),
    ("designer", include_str!("../agents/designer/AGENT.md")),
    ("tester", include_str!("../agents/tester/AGENT.md")),
    ("reviewer", include_str!("../agents/reviewer/AGENT.md")),
    ("researcher", include_str!("../agents/researcher/AGENT.md")),
    ("scribe", include_str!("../agents/scribe/AGENT.md")),
    ("scout", include_str!("../agents/scout/AGENT.md")),
    ("ripple", include_str!("../agents/ripple/AGENT.md")),
    ("aegis", include_str!("../agents/aegis/AGENT.md")),
    ("bursar", include_str!("../agents/bursar/AGENT.md")),
    ("referee", include_str!("../agents/referee/AGENT.md")),
    ("customs", include_str!("../agents/customs/AGENT.md")),
    ("anchor", include_str!("../agents/anchor/AGENT.md")),
    ("curator", include_str!("../agents/curator/AGENT.md")),
];

fn user_copy(rel: &str) -> Option<String> {
    let dir = config::path().parent()?.join("agents");
    std::fs::read_to_string(dir.join(rel)).ok()
}

fn agent_md(role: &str) -> Option<String> {
    user_copy(&format!("{role}/AGENT.md")).or_else(|| {
        BUILTIN
            .iter()
            .find(|(r, _)| *r == role)
            .map(|(_, s)| s.to_string())
    })
}

/// Everything an agent is told before its task: the shared rules, its role,
/// who else is on the team, and what the owner already decided.
/// `lessons` is the project's LESSONS.md, written from ripple's `LESSON:` lines.
/// `savings` is the bursar's rules for this agent's role.
pub fn instructions(
    me: &AgentDef,
    team: &[AgentDef],
    decided: &[String],
    lessons: &str,
    savings: &str,
) -> String {
    let rules = user_copy("TEAM.md").unwrap_or_else(|| TEAM.to_string());
    let role = agent_md(&me.role).unwrap_or_else(|| {
        format!("# {}\n\nThere is no AGENT.md for this role yet. Follow the shared rules above and the boss's instructions.\n", me.role)
    });
    let mut out = format!("{rules}\n---\n\n{role}\n---\n\n# Your team right now\n\n");
    out += &format!(
        "You are **{}**, the {}, running on {} `{}`.\n\n",
        me.name, me.role, me.vendor, me.model
    );
    out += "| Name | Role | Vendor and model | Job |\n|---|---|---|---|\n";
    for a in team {
        out += &format!(
            "| {} | {} | {} `{}` | {} |\n",
            a.name, a.role, a.vendor, a.model, a.job
        );
    }
    out += "\n# What the owner already decided\n\n";
    if decided.is_empty() {
        out += "Nothing yet.\n";
    }
    for d in decided {
        out += &format!("- {d}\n");
    }
    if !lessons.trim().is_empty() {
        out += "\n";
        out += lessons.trim_end();
        out += "\n";
    }
    if !savings.trim().is_empty() {
        out += "\n# Savings rules\n\nThe bursar's rules for spending fewer tokens. Each one is measured; follow them unless one would make your work worse, and then say so in your report.\n\n";
        out += savings;
    }
    out
}

#[derive(Debug, PartialEq)]
pub enum Line {
    Ask {
        text: String,
        options: Vec<String>,
    },
    Report(String),
    Learned {
        what: String,
        source: Option<String>,
    },
    /// ripple: a commit that must not be merged until its break is fixed.
    Hold {
        commit: String,
        reason: String,
    },
    Release(String),
    /// bursar: a savings rule for a role, or "all"
    Save {
        scope: String,
        text: String,
    },
    Drop(String),
    /// ripple: a rule for the project's LESSONS.md.
    Lesson(String),
    /// The scout's proposal to move an agent to another model.
    Recommend {
        agent: String,
        vendor: String,
        model: String,
        why: String,
        source: Option<String>,
    },
}

/// A message from one agent to another: `MSG <from> -> <to> [re #N]`, `key: value`
/// lines, `END`. See agents/TEAM.md.
#[derive(Debug, PartialEq, Clone)]
pub struct Card {
    pub to: String,
    pub re: Option<u32>,
    pub kind: String,
    pub fields: Vec<(String, String)>,
}

impl Card {
    pub fn get(&self, key: &str) -> &str {
        self.fields
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
            .unwrap_or("")
    }

    /// Fields this kind of card must carry but does not.
    pub fn missing(&self) -> Vec<&'static str> {
        let need: &[&'static str] = match self.kind.as_str() {
            "bug" => &["title", "repro", "expected", "actual", "done when"],
            "fixed" => &["title", "commit"],
            _ => &["title"],
        };
        need.iter()
            .copied()
            .filter(|k| self.get(k).is_empty())
            .collect()
    }
}

/// Pull the MSG blocks out of an agent's text. Returns the cards and the other lines.
pub fn cards(text: &str) -> (Vec<Card>, Vec<String>) {
    let mut cards = vec![];
    let mut rest = vec![];
    let mut open: Option<Card> = None;
    for raw in text.lines() {
        let line = raw.trim().trim_matches(['*', '`']).trim();
        if let Some(card) = open.as_mut() {
            if line == "END" {
                cards.push(open.take().unwrap());
                continue;
            }
            if line.is_empty() || line.starts_with("```") {
                continue;
            }
            match line.split_once(':') {
                Some((k, v))
                    if k.len() <= 20 && k.chars().all(|c| c.is_ascii_alphabetic() || c == ' ') =>
                {
                    let (k, v) = (k.trim().to_lowercase(), v.trim().to_string());
                    if k == "kind" {
                        card.kind = v.to_lowercase();
                    } else {
                        card.fields.push((k, v));
                    }
                }
                _ => match card.fields.last_mut() {
                    Some((_, v)) => {
                        v.push(' ');
                        v.push_str(line);
                    }
                    None => card.fields.push(("text".into(), line.into())),
                },
            }
            continue;
        }
        if let Some(head) = line.strip_prefix("MSG ").and_then(|h| h.split_once("->")) {
            let target = head.1.trim();
            let to = target
                .split_whitespace()
                .next()
                .unwrap_or("")
                .trim_matches(|c: char| !c.is_alphanumeric() && c != '-')
                .to_string();
            let re = target.split('#').nth(1).and_then(|n| {
                n.chars()
                    .take_while(char::is_ascii_digit)
                    .collect::<String>()
                    .parse()
                    .ok()
            });
            open = Some(Card {
                to,
                re,
                kind: "note".into(),
                fields: vec![],
            });
            continue;
        }
        rest.push(raw.to_string());
    }
    // a card the agent forgot to close still counts
    cards.extend(open);
    (cards, rest)
}

/// The project's own notes for every agent: curator's MAP.md and the LESSONS.md
/// that ripple, aegis, the referee and customs write.
pub fn project_notes(map: &str, lessons: &str) -> String {
    let mut s = String::new();
    if !map.trim().is_empty() {
        s +=
            "# Project map\n\nThe curator's map of this project. Read it instead of exploring.\n\n";
        s += map.trim();
        s += "\n";
    }
    if !lessons.trim().is_empty() {
        s += "\n# Lessons from earlier breaks in this project\n\nRules the team wrote after catching a mistake. Follow them.\n\n";
        s += lessons.trim();
        s += "\n";
    }
    s
}

/// One line of agent output: a protocol line, or None for ordinary text.
pub fn protocol(line: &str) -> Option<Line> {
    let line = line.trim().trim_start_matches(['*', '`']);
    let split = |rest: &str| {
        rest.split('|')
            .map(|s| s.trim().trim_matches(['*', '`']).trim().to_string())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
    };
    if let Some(rest) = line.strip_prefix("ASK:") {
        let mut parts = split(rest);
        if parts.is_empty() {
            return None;
        }
        let text = parts.remove(0);
        let options = if parts.len() >= 2 {
            parts
        } else {
            vec!["yes".into(), "no".into()]
        };
        return Some(Line::Ask { text, options });
    }
    if let Some(rest) = line.strip_prefix("HOLD:") {
        let mut p = split(rest).into_iter();
        return Some(Line::Hold {
            commit: p.next()?,
            reason: p.next().unwrap_or_default(),
        });
    }
    if let Some(rest) = line.strip_prefix("SAVE:") {
        let mut p = split(rest).into_iter();
        let (scope, text) = (p.next()?, p.next()?);
        return Some(Line::Save {
            scope: scope.to_lowercase(),
            text,
        });
    }
    if let Some(rest) = line.strip_prefix("DROP:") {
        return split(rest)
            .into_iter()
            .next()
            .map(|id| Line::Drop(id.to_uppercase()));
    }
    if let Some(rest) = line.strip_prefix("RELEASE:") {
        return split(rest).into_iter().next().map(Line::Release);
    }
    if let Some(rest) = line.strip_prefix("LESSON:") {
        let text = rest.trim().trim_matches(['*', '`']).trim();
        return (!text.is_empty()).then(|| Line::Lesson(text.to_string()));
    }
    if let Some(rest) = line.strip_prefix("RECOMMEND:") {
        let mut p = split(rest).into_iter();
        let (agent, vendor, model) = (p.next()?, p.next()?, p.next()?);
        return Some(Line::Recommend {
            agent,
            vendor,
            model,
            why: p.next().unwrap_or_default(),
            source: p.next(),
        });
    }
    if let Some(rest) = line.strip_prefix("REPORT:") {
        return Some(Line::Report(rest.trim().to_string()));
    }
    if let Some(rest) = line.strip_prefix("LEARNED:") {
        let mut parts = split(rest).into_iter();
        let what = parts.next()?;
        return Some(Line::Learned {
            what,
            source: parts.next(),
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[test]
    fn every_default_agent_has_a_role_file() {
        for a in Config::default().agents {
            assert!(
                BUILTIN.iter().any(|(r, _)| *r == a.role),
                "no AGENT.md for {}",
                a.role
            );
        }
    }

    #[test]
    fn instructions_name_the_agent_and_the_team() {
        let c = Config::default();
        let me = c.agents.iter().find(|a| a.name == "builder-2").unwrap();
        let s = instructions(
            me,
            &c.agents,
            &["commits: one line".into()],
            &project_notes("run with cargo run", "- check every caller"),
            "- grep first (S1)\n",
        );
        assert!(s.starts_with("# Working in orda"));
        assert!(s.contains("# Builder"));
        assert!(s.contains("You are **builder-2**, the builder"));
        assert!(s.contains("| tester | tester |"));
        assert!(s.contains("- commits: one line"));
        assert!(
            s.contains("# Lessons from earlier breaks")
                && s.contains("- check every caller\n")
                && s.contains("# Project map")
        );
        assert!(s.ends_with("- grep first (S1)\n"));
    }

    #[test]
    fn cards_parse() {
        let text = "Found one.\nMSG tester -> builder\nkind: bug\nseverity: high\ntitle: turn.diff crashes\nrepro: cargo test codex\nexpected: no panic\nactual: panic at codex.rs:48\n  when the item is new\ndone when: test passes\nEND\nMoving on.\n**MSG builder -> tester re #12**\nkind: fixed\ntitle: handled\ncommit: b71c0e2\nEND";
        let (cards, rest) = cards(text);
        assert_eq!(rest, ["Found one.", "Moving on."]);
        assert_eq!(cards.len(), 2);
        let bug = &cards[0];
        assert_eq!(
            (bug.to.as_str(), bug.kind.as_str(), bug.re),
            ("builder", "bug", None)
        );
        assert_eq!(
            bug.get("actual"),
            "panic at codex.rs:48 when the item is new"
        );
        assert!(bug.missing().is_empty());
        let fixed = &cards[1];
        assert_eq!(
            (fixed.to.as_str(), fixed.kind.as_str(), fixed.re),
            ("tester", "fixed", Some(12))
        );
        let (c, _) = super::cards("MSG tester -> builder\nkind: bug\ntitle: x");
        assert_eq!(c[0].missing(), ["repro", "expected", "actual", "done when"]);
    }

    #[test]
    fn protocol_lines() {
        assert_eq!(
            protocol("ASK: SQLite or TOML? | TOML file | SQLite"),
            Some(Line::Ask {
                text: "SQLite or TOML?".into(),
                options: vec!["TOML file".into(), "SQLite".into()]
            })
        );
        assert_eq!(
            protocol("**ASK:** Ship it?"),
            Some(Line::Ask {
                text: "Ship it?".into(),
                options: vec!["yes".into(), "no".into()]
            })
        );
        assert_eq!(
            protocol("ASK: Ship it?"),
            Some(Line::Ask {
                text: "Ship it?".into(),
                options: vec!["yes".into(), "no".into()]
            })
        );
        assert_eq!(
            protocol("REPORT: a3f9c21, 14/20 passing"),
            Some(Line::Report("a3f9c21, 14/20 passing".into()))
        );
        assert_eq!(
            protocol("LEARNED: Frame::area replaces size | https://ratatui.rs/x"),
            Some(Line::Learned {
                what: "Frame::area replaces size".into(),
                source: Some("https://ratatui.rs/x".into())
            })
        );
        assert_eq!(protocol("I will ASK: later"), None);
        assert_eq!(
            protocol("RECOMMEND: tester | codex | gpt-7 | 71% vs 58% | https://x.io"),
            Some(Line::Recommend {
                agent: "tester".into(),
                vendor: "codex".into(),
                model: "gpt-7".into(),
                why: "71% vs 58%".into(),
                source: Some("https://x.io".into())
            })
        );
        assert_eq!(protocol("RECOMMEND: tester | codex"), None);
        assert_eq!(
            protocol("HOLD: 9e1b0d4 | lr() pads one column too far"),
            Some(Line::Hold {
                commit: "9e1b0d4".into(),
                reason: "lr() pads one column too far".into()
            })
        );
        assert_eq!(
            protocol("RELEASE: 9e1b0d4"),
            Some(Line::Release("9e1b0d4".into()))
        );
        assert_eq!(
            protocol("LESSON: check every caller"),
            Some(Line::Lesson("check every caller".into()))
        );
        assert_eq!(protocol("LESSON:"), None);
        assert_eq!(
            protocol("SAVE: Tester | read only failing output"),
            Some(Line::Save {
                scope: "tester".into(),
                text: "read only failing output".into()
            })
        );
        assert_eq!(protocol("DROP: s3"), Some(Line::Drop("S3".into())));
    }
}
