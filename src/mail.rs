//! Messages between agents: typed cards, threads that count fix rounds, and the
//! flights the team panel animates from sender to receiver.

use crate::roles::Card;
use crate::theme::Theme;
use ratatui::style::Color;
use std::time::{Duration, Instant};

/// A bug that comes back this many times goes to the boss and the advisor.
pub const ROUNDS: usize = 3;

/// How long a message takes to travel between two cards on screen.
pub const FLIGHT: Duration = Duration::from_millis(1400);

pub struct Message {
    pub id: u32,
    /// The first message of the conversation this one belongs to.
    pub thread: u32,
    pub from: String,
    pub to: String,
    pub card: Card,
}

pub struct Flight {
    pub from: String,
    pub to: String,
    pub label: String,
    pub kind: String,
    pub start: Instant,
    /// Delivered to the receiver when the flight lands.
    pub msg: u32,
}

impl Flight {
    /// 0.0 at the sender, 1.0 at the receiver.
    pub fn progress(&self) -> f64 {
        (self.start.elapsed().as_secs_f64() / FLIGHT.as_secs_f64()).min(1.0)
    }
}

/// Cards that need the receiver to act start a run. `done` and `note` only inform:
/// they wait for the receiver's next run, so two polite agents cannot thank each
/// other forever.
pub fn wakes(kind: &str) -> bool {
    matches!(
        kind,
        "task" | "bug" | "review" | "question" | "escalation" | "fixed" | "failure"
    )
}

pub fn color(t: &Theme, kind: &str) -> Color {
    match kind {
        "bug" | "escalation" | "failure" => t.bad,
        "task" => t.claude,
        "fixed" | "done" => t.ok,
        "review" => t.codex,
        "question" => t.ask,
        _ => t.bright,
    }
}

/// A card as text, the way agents write them.
pub fn render(m: &Message) -> String {
    let mut s = format!("MSG {} -> {} #{}", m.from, m.to, m.id);
    if m.thread != m.id {
        s += &format!(" re #{}", m.thread);
    }
    s += &format!("\nkind: {}\n", m.card.kind);
    for (k, v) in &m.card.fields {
        s += &format!("{k}: {v}\n");
    }
    s + "END"
}

/// The task given to the receiver: the whole conversation, newest last.
pub fn prompt(thread: &[&Message], to: &str) -> String {
    let last = thread.last().expect("a thread has at least one message");
    let mut s = format!(
        "You have a message from {} (#{}). The whole conversation so far, oldest first:\n\n",
        last.from, last.id
    );
    for m in thread {
        s += &render(m);
        s += "\n\n";
    }
    s += &format!(
        "Deal with it as your role describes, then answer {} with a MSG card `re #{}` as agents/TEAM.md shows. You are {to}.\n",
        last.from, last.thread
    );
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_carries_the_thread() {
        let card = |kind: &str| Card {
            to: "x".into(),
            re: None,
            kind: kind.into(),
            fields: vec![("title".into(), "t".into())],
        };
        let a = Message {
            id: 12,
            thread: 12,
            from: "tester".into(),
            to: "builder".into(),
            card: card("bug"),
        };
        let b = Message {
            id: 13,
            thread: 12,
            from: "builder".into(),
            to: "tester".into(),
            card: card("fixed"),
        };
        let p = prompt(&[&a, &b], "tester");
        assert!(p.contains("MSG tester -> builder #12\nkind: bug\ntitle: t\nEND"));
        assert!(p.contains("MSG builder -> tester #13 re #12"));
        assert!(p.contains("answer builder with a MSG card `re #12`"));
    }
}
