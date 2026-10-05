use super::{SPINNER, bar, human, lr, panel};
use crate::app::{Agent, App, Status};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Paragraph};

const CARD_H: u16 = 7;
/// The shortest worker card: status, detail and the footer.
const MIN_CARD_H: u16 = 5;
/// A card for an agent with nothing going on: title and one line.
const SLIM_H: u16 = 3;

pub fn draw(f: &mut Frame, app: &App, area: Rect) {
    let t = &app.theme;
    let working = app.agents.iter().filter(|a| a.status.busy()).count();
    let title = vec![
        Span::styled(
            "team",
            Style::new().fg(t.bright).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("  {} agents · {working} working", app.agents.len()),
            Style::new().fg(t.dim),
        ),
    ];
    // while a message is in the air, the title bar says what it is
    let right = match app.flights.last() {
        Some(fl) => Line::from(vec![
            Span::styled(
                format!(" {} -> {} ", fl.from, fl.to),
                Style::new().fg(t.bright),
            ),
            Span::styled(
                format!(" {} ", fl.label),
                Style::new()
                    .fg(t.bg)
                    .bg(crate::mail::color(t, &fl.kind))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
        ]),
        None => Line::from(Span::styled(
            " every agent is wired to every other ",
            Style::new().fg(t.dim),
        )),
    };
    let block = panel(app, title, String::new(), t.line, false).title_top(right.right_aligned());
    let inner = block.inner(area);
    f.render_widget(block, area);
    let inner = Rect {
        x: inner.x + 1,
        width: inner.width.saturating_sub(2),
        y: inner.y + 1,
        height: inner.height.saturating_sub(1),
    };

    let boss = app.agents.iter().position(|a| a.def.role == "boss");
    let advisor = app.agents.iter().position(|a| a.def.role == "advisor");
    let workers: Vec<usize> = (0..app.agents.len())
        .filter(|i| Some(*i) != boss && Some(*i) != advisor)
        .collect();

    let [top, wire, rest] = Layout::vertical([
        Constraint::Length(CARD_H),
        Constraint::Length(2),
        Constraint::Fill(1),
    ])
    .areas(inner);
    let wire_y = wire.y + 1;
    let mut net = Net::default();
    // agent index -> the lane cell right outside its card, where its messages leave and arrive
    let mut ports: Vec<(usize, (u16, u16))> = vec![];

    // boss row: advisor | boss | you, each hangs down to the connector row
    let mut slots: Vec<(Option<usize>, u16)> = vec![];
    if advisor.is_some() {
        slots.push((advisor, 10));
    }
    slots.push((boss, 14));
    slots.push((None, 10));
    let cols = Layout::horizontal(slots.iter().map(|(_, w)| Constraint::Fill(*w)))
        .spacing(2)
        .split(top);
    let mut drops = vec![];
    for ((idx, _), r) in slots.iter().zip(cols.iter()) {
        match idx {
            Some(i) => agent_card(f, app, &app.agents[*i], *r),
            None => you_card(f, app, *r),
        }
        let x = r.x + r.width / 2;
        net.line((x, wire.y), (x, wire_y));
        // the boss card has a thick border, so its junction is thick too
        net.socket(
            (x, r.bottom() - 1),
            if boss.is_some() && *idx == boss {
                '┳'
            } else {
                '┬'
            },
        );
        drops.push(x);
        if let Some(i) = idx {
            ports.push((*i, (x, wire.y)));
        }
    }

    // workers: two to a row (three on wide screens), each with a stub into the lane beside it
    let per_row = if inner.width >= 110 {
        3
    } else if inner.width >= 56 {
        2
    } else {
        1
    };
    // a bigger team gets shorter worker cards before any card is left out; when even
    // that is not enough, rows where nobody is working shrink to slim cards
    let all: Vec<&[usize]> = workers.chunks(per_row).collect();
    let lively = |row: &[usize]| row.iter().any(|i| lively(app, &app.agents[*i]));
    let total = |h: u16, slim: bool| -> u16 {
        all.iter()
            .map(|r| {
                if slim && !lively(r) {
                    SLIM_H + 1
                } else {
                    h + 1
                }
            })
            .sum()
    };
    let fits = |h: u16, slim: bool| total(h, slim) <= rest.height + 1;
    let (card_h, slim) = (MIN_CARD_H..=CARD_H)
        .rev()
        .map(|h| (h, false))
        .chain((MIN_CARD_H..=CARD_H).rev().map(|h| (h, true)))
        .find(|(h, slim)| fits(*h, *slim))
        .unwrap_or((MIN_CARD_H, true));
    let mut used = 0;
    let mut heights = vec![];
    for r in &all {
        let h = if slim && !lively(r) { SLIM_H } else { card_h };
        if used + h > rest.height {
            break;
        }
        used += h + 1;
        heights.push(h);
    }
    let rows: Vec<&[usize]> = all.iter().take(heights.len().max(1)).copied().collect();
    let row_rects = Layout::vertical(heights.iter().map(|h| Constraint::Length(*h)))
        .spacing(1)
        .split(rest);
    // lane x -> lowest row a card joins it
    let mut lanes: Vec<(u16, u16)> = vec![];
    for (row, rr) in rows.iter().zip(row_rects.iter()) {
        let cells = Layout::horizontal(vec![Constraint::Fill(1); per_row])
            .spacing(3)
            .split(*rr);
        for (c, (i, cell)) in row.iter().zip(cells.iter()).enumerate() {
            agent_card(f, app, &app.agents[*i], *cell);
            let y = cell.y + cell.height / 2;
            // into the gap on the right, except the last column, which uses the gap on its left
            let (border, start, lane, glyph) = if per_row == 1 {
                (cell.right() - 1, cell.right(), cell.right(), '├')
            } else if c + 1 < per_row {
                (cell.right() - 1, cell.right(), cell.right() + 1, '├')
            } else {
                (cell.x, cell.x - 1, cell.x - 2, '┤')
            };
            net.line((start, y), (lane, y));
            net.socket((border, y), glyph);
            ports.push((*i, (start, y)));
            match lanes.iter_mut().find(|(x, _)| *x == lane) {
                Some(l) => l.1 = l.1.max(y),
                None => lanes.push((lane, y)),
            }
        }
    }
    for (x, bottom) in &lanes {
        net.line((*x, wire_y), (*x, *bottom));
    }
    let xs: Vec<u16> = drops
        .iter()
        .chain(lanes.iter().map(|(x, _)| x))
        .copied()
        .collect();
    if let (Some(lo), Some(hi)) = (xs.iter().min(), xs.iter().max()) {
        net.line((*lo, wire_y), (*hi, wire_y));
    }

    let shown: usize = rows.iter().map(|r| r.len()).sum();
    if shown < workers.len() {
        let more = format!(" +{} more below ", workers.len() - shown);
        let r = Rect {
            x: area.right().saturating_sub(more.len() as u16 + 2),
            y: area.bottom() - 1,
            width: more.len() as u16,
            height: 1,
        };
        f.render_widget(
            Paragraph::new(Span::styled(more, Style::new().fg(t.dim))),
            r,
        );
    }

    net.draw(f, t.line);
    draw_flights(f, app, &net, &ports);
}

const N: u8 = 1;
const E: u8 = 2;
const S: u8 = 4;
const W: u8 = 8;

/// The wiring between cards: every lane cell and which neighbours it joins.
#[derive(Default)]
struct Net {
    cells: std::collections::HashMap<(u16, u16), u8>,
    /// Junctions cut into card borders where a line meets the card.
    sockets: Vec<((u16, u16), char)>,
}

impl Net {
    /// A straight line from `a` to `b` (same row or same column).
    fn line(&mut self, a: (u16, u16), b: (u16, u16)) {
        self.cells.entry(a).or_insert(0);
        let mut cur = a;
        while cur != b {
            let (next, out, back) = if cur.0 != b.0 {
                if b.0 > cur.0 {
                    ((cur.0 + 1, cur.1), E, W)
                } else {
                    ((cur.0 - 1, cur.1), W, E)
                }
            } else if b.1 > cur.1 {
                ((cur.0, cur.1 + 1), S, N)
            } else {
                ((cur.0, cur.1 - 1), N, S)
            };
            *self.cells.entry(cur).or_insert(0) |= out;
            *self.cells.entry(next).or_insert(0) |= back;
            cur = next;
        }
    }

    fn socket(&mut self, at: (u16, u16), glyph: char) {
        self.sockets.push((at, glyph));
    }

    fn draw(&self, f: &mut Frame, color: Color) {
        let buf = f.buffer_mut();
        for (&(x, y), &d) in &self.cells {
            let g = match d {
                d if d == N | E | S | W => "┼",
                d if d == N | S | E => "├",
                d if d == N | S | W => "┤",
                d if d == E | W | S => "┬",
                d if d == E | W | N => "┴",
                d if d == S | E => "┌",
                d if d == S | W => "┐",
                d if d == N | E => "└",
                d if d == N | W => "┘",
                d if d & (N | S) != 0 => "│",
                _ => "─",
            };
            buf[(x, y)].set_symbol(g).set_fg(color);
        }
        // the border keeps its own colour, only the shape changes
        for &((x, y), g) in &self.sockets {
            buf[(x, y)].set_char(g);
        }
    }

    /// The shortest way along the wires from one cell to another.
    fn route(&self, from: (u16, u16), to: (u16, u16)) -> Option<Vec<(u16, u16)>> {
        use std::collections::{HashMap, VecDeque};
        let mut prev: HashMap<(u16, u16), (u16, u16)> = HashMap::new();
        let mut queue = VecDeque::from([from]);
        prev.insert(from, from);
        while let Some(c) = queue.pop_front() {
            if c == to {
                let mut path = vec![c];
                let mut cur = c;
                while cur != from {
                    cur = prev[&cur];
                    path.push(cur);
                }
                path.reverse();
                return Some(path);
            }
            let d = *self.cells.get(&c).unwrap_or(&0);
            let steps = [(N, 0, -1), (E, 1, 0), (S, 0, 1), (W, -1, 0)];
            for (bit, dx, dy) in steps {
                if d & bit == 0 {
                    continue;
                }
                let n = ((c.0 as i32 + dx) as u16, (c.1 as i32 + dy) as u16);
                if let std::collections::hash_map::Entry::Vacant(e) = prev.entry(n) {
                    e.insert(c);
                    queue.push_back(n);
                }
            }
        }
        None
    }
}

fn draw_flights(f: &mut Frame, app: &App, net: &Net, ports: &[(usize, (u16, u16))]) {
    let t = &app.theme;
    for fl in &app.flights {
        let port = |name: &str| {
            let i = app.agent_index(name)?;
            ports.iter().find(|(k, _)| *k == i).map(|(_, p)| *p)
        };
        let Some(pts) = port(&fl.from)
            .zip(port(&fl.to))
            .and_then(|(a, b)| net.route(a, b))
        else {
            continue;
        };
        let head = ((fl.progress() * (pts.len() - 1) as f64).round() as usize).min(pts.len() - 1);
        let trail = app.accent_of(&fl.from);
        let kind = crate::mail::color(t, &fl.kind);
        let buf = f.buffer_mut();
        for (k, ch) in ["▓", "▒", "░"].iter().enumerate() {
            if let Some(&(x, y)) = head.checked_sub(k + 1).and_then(|i| pts.get(i)) {
                buf[(x, y)].set_symbol(ch).set_fg(trail);
            }
        }
        let (x, y) = pts[head];
        buf[(x, y)].set_symbol("█").set_fg(kind);
    }
}

/// An agent worth a full card: working, asking, failed, or just got a message.
fn lively(app: &App, a: &Agent) -> bool {
    a.status.busy()
        || matches!(a.status, Status::Asking | Status::Failed)
        || a.flash
            .as_ref()
            .is_some_and(|f| f.0.elapsed() < crate::app::FLASH)
        || app
            .flights
            .iter()
            .any(|f| f.to == a.def.name || f.from == a.def.name)
}

fn badge(app: &App, a: &Agent) -> Span<'static> {
    let t = &app.theme;
    let (label, color) = match a.status {
        Status::Running if a.def.always_on => ("always on", t.ok),
        Status::Running => ("running", t.ok),
        Status::Searching => ("searching", t.gemini),
        Status::Asking => ("asking", t.ask),
        Status::Queued => ("queued", t.dim),
        Status::Idle => ("idle", t.dim),
        Status::Done => ("done", t.ok),
        Status::Failed => ("failed", t.bad),
    };
    Span::styled(format!(" {label} "), Style::new().fg(color).bg(t.badge))
}

fn agent_card(f: &mut Frame, app: &App, a: &Agent, area: Rect) {
    let t = &app.theme;
    let accent = t.accent(&a.def.vendor, &a.def.model);
    let active = matches!(
        a.status,
        Status::Running | Status::Searching | Status::Asking
    );
    let lit = a
        .flash
        .as_ref()
        .filter(|f| f.0.elapsed() < crate::app::FLASH);
    let border = match a.status {
        _ if lit.is_some() => crate::mail::color(t, &lit.unwrap().1),
        Status::Failed => t.bad,
        _ if active => accent,
        _ => t.line,
    };
    let title_color = if active || a.status == Status::Done {
        accent
    } else {
        t.dim
    };
    // the model already names the vendor; the effort goes first when the border is short
    let room = (area.width as usize).saturating_sub(a.def.name.len() + 8);
    let mut right = a.def.model.clone();
    // on a fallback the title says so instead of showing the effort
    let extra = if a.primary.is_some() {
        "fallback"
    } else {
        a.def.effort.as_str()
    };
    if !extra.is_empty() && right.len() + extra.len() + 3 <= room {
        right = format!("{right} · {extra}");
    }
    let block = Block::bordered()
        .border_type(if a.def.role == "boss" {
            BorderType::Thick
        } else {
            BorderType::Rounded
        })
        .border_style(Style::new().fg(border))
        .title_top(Line::from(Span::styled(
            format!(" {} ", a.def.name),
            Style::new().fg(title_color).add_modifier(Modifier::BOLD),
        )))
        .title_top(
            Line::from(Span::styled(format!(" {right} "), Style::new().fg(t.dim))).right_aligned(),
        );
    let inner = block.inner(area);
    f.render_widget(block, area);
    let inner = Rect {
        x: inner.x + 1,
        width: inner.width.saturating_sub(2),
        ..inner
    };
    let w = inner.width;

    let mut now = vec![];
    if a.status.busy() {
        now.push(Span::styled(
            format!("{} ", SPINNER[app.frame % 4]),
            Style::new().fg(accent),
        ));
    }
    let now_text = if a.now.is_empty() {
        a.def.job.clone()
    } else {
        a.now.clone()
    };
    now.push(Span::styled(
        now_text,
        Style::new().fg(if active { t.bright } else { t.dim }),
    ));

    let detail: Line = if let (Some(burn), "bursar") = (&app.burn, a.def.role.as_str()) {
        let c = if burn.starts_with('-') { t.ok } else { t.bad };
        lr(
            vec![Span::styled(
                "tokens per run vs last week ",
                Style::new().fg(t.dim),
            )],
            vec![Span::styled(
                burn.clone(),
                Style::new().fg(c).add_modifier(Modifier::BOLD),
            )],
            w,
        )
    } else if a.def.role == "boss" {
        let frac = a.context as f64 / 1_000_000.0;
        let mut s = vec![Span::styled("context ", Style::new().fg(t.dim))];
        s.extend(bar(frac, 10, accent, t.line));
        lr(
            s,
            vec![
                Span::styled(human(a.context), Style::new().fg(t.bright)),
                Span::styled("/1M", Style::new().fg(t.dim)),
            ],
            w,
        )
    } else if let Some((p, n)) = a.tests {
        lr(
            bar(
                p as f64 / n as f64,
                (w as usize).saturating_sub(8).min(20),
                t.ok,
                t.line,
            ),
            vec![
                Span::styled(format!("{p}"), Style::new().fg(t.bright)),
                Span::styled(format!("/{n} "), Style::new().fg(t.dim)),
            ],
            w,
        )
    } else {
        Line::from(Span::styled(a.def.job.clone(), Style::new().fg(t.dim)))
    };

    let mut lines = vec![lr(now, vec![badge(app, a)], w), detail];
    let out: Vec<&String> = a.output.iter().rev().take(2).collect();
    for l in out.into_iter().rev() {
        lines.push(Line::from(Span::styled(l.clone(), Style::new().fg(t.text))));
    }
    // a short card drops output lines first; the status, the detail and the footer stay
    let room = inner.height.saturating_sub(1) as usize;
    lines.truncate(room.max(1));
    while lines.len() < room {
        lines.push(Line::default());
    }
    let tokens = if a.tokens() > 0 {
        format!("{} tok", human(a.tokens()))
    } else {
        String::new()
    };
    lines.push(lr(
        vec![Span::styled(a.task.clone(), Style::new().fg(t.dim))],
        vec![Span::styled(tokens, Style::new().fg(t.dim))],
        w,
    ));
    let lines: Vec<Line> = lines
        .into_iter()
        .map(|l| Line::from(super::fit(l.spans, w as usize)))
        .collect();
    f.render_widget(Paragraph::new(lines), inner);
}

fn you_card(f: &mut Frame, app: &App, area: Rect) {
    let t = &app.theme;
    let n = app.questions.len();
    let waiting: Vec<&str> = app
        .questions
        .iter()
        .filter(|q| q.guard_cmd.is_some())
        .map(|q| q.from.as_str())
        .collect();
    let border: Color = if n > 0 { t.ask } else { t.line };
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(border))
        .title_top(Line::from(Span::styled(
            " you ",
            Style::new().fg(t.bright).add_modifier(Modifier::BOLD),
        )))
        .title_top(Line::from(Span::styled(" owner ", Style::new().fg(t.dim))).right_aligned());
    let inner = block.inner(area);
    f.render_widget(block, area);
    let inner = Rect {
        x: inner.x + 1,
        width: inner.width.saturating_sub(2),
        ..inner
    };
    let w = inner.width;
    let first = if n == 0 {
        lr(
            vec![Span::styled("no questions", Style::new().fg(t.dim))],
            vec![],
            w,
        )
    } else {
        lr(
            vec![Span::styled(
                format!("{n} question{} waiting", if n == 1 { "" } else { "s" }),
                Style::new().fg(t.ask),
            )],
            vec![Span::styled(" q ", Style::new().fg(t.ask).bg(t.badge))],
            w,
        )
    };
    let second = if waiting.is_empty() {
        Span::styled("no one waits on you", Style::new().fg(t.dim))
    } else {
        Span::styled(
            format!("{} waits on a guard answer", waiting.join(", ")),
            Style::new().fg(t.ask),
        )
    };
    let lines = vec![
        first,
        Line::from(second),
        Line::from(Span::styled("answers are saved", Style::new().fg(t.dim))),
        Line::from(Span::styled("per project or all", Style::new().fg(t.dim))),
        lr(
            vec![],
            vec![Span::styled(
                format!("{} remembered", app.remembered.len()),
                Style::new().fg(t.dim),
            )],
            w,
        ),
    ];
    f.render_widget(Paragraph::new(lines), inner);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn net_routes_along_the_wires() {
        // two cards hang from a connector row, two workers stub into a lane at x=20
        let mut net = Net::default();
        net.line((5, 0), (5, 1));
        net.line((30, 0), (30, 1));
        net.line((5, 1), (30, 1));
        net.line((20, 1), (20, 12));
        net.line((18, 6), (20, 6));
        net.line((22, 12), (20, 12));
        let p = net.route((18, 6), (22, 12)).unwrap();
        assert_eq!((p.first(), p.last()), (Some(&(18, 6)), Some(&(22, 12))));
        for w in p.windows(2) {
            let d = (w[0].0 as i32 - w[1].0 as i32).abs() + (w[0].1 as i32 - w[1].1 as i32).abs();
            assert_eq!(d, 1, "every step moves one cell along a wire");
        }
        // up the lane, along the connector row, down to the card at x=30
        let p = net.route((18, 6), (30, 0)).unwrap();
        assert!(p.contains(&(20, 1)) && p.contains(&(30, 1)));
        assert!(net.route((18, 6), (99, 99)).is_none());
    }
}
