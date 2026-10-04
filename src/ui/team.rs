use super::{SPINNER, bar, human, lr, panel};
use crate::app::{Agent, App, Status};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Paragraph};

const CARD_H: u16 = 7;

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
    let block = panel(
        app,
        title,
        "agents report to the boss".into(),
        t.line,
        false,
    );
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

    // boss row: advisor | boss | you
    let mut slots: Vec<(Option<usize>, u16)> = vec![];
    if advisor.is_some() {
        slots.push((advisor, 10));
    }
    slots.push((boss, 14));
    slots.push((None, 10));
    let cols = Layout::horizontal(slots.iter().map(|(_, w)| Constraint::Fill(*w)))
        .spacing(2)
        .split(top);
    let mut boss_rect = cols[0];
    for ((idx, _), r) in slots.iter().zip(cols.iter()) {
        match idx {
            Some(i) => {
                if Some(*i) == boss {
                    boss_rect = *r;
                }
                agent_card(f, app, &app.agents[*i], *r);
            }
            None => you_card(f, app, *r),
        }
    }

    // workers, three to a row
    let per_row = if inner.width >= 90 {
        3
    } else if inner.width >= 56 {
        2
    } else {
        1
    };
    let fit_rows = ((rest.height + 1) / (CARD_H + 1)).max(1) as usize;
    let rows: Vec<&[usize]> = workers.chunks(per_row).take(fit_rows).collect();
    let row_rects = Layout::vertical(vec![Constraint::Length(CARD_H); rows.len()])
        .spacing(1)
        .split(rest);
    let mut first_centers = vec![];
    for (ri, (row, rr)) in rows.iter().zip(row_rects.iter()).enumerate() {
        let cells = Layout::horizontal(vec![Constraint::Fill(1); per_row])
            .spacing(2)
            .split(*rr);
        for (i, cell) in row.iter().zip(cells.iter()) {
            agent_card(f, app, &app.agents[*i], *cell);
            if ri == 0 {
                first_centers.push(cell.x + cell.width / 2);
            }
        }
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

    draw_wire(
        f,
        app,
        boss_rect.x + boss_rect.width / 2,
        &first_centers,
        wire,
    );
}

/// The connector from the boss card down to the first row of workers.
fn draw_wire(f: &mut Frame, app: &App, boss_x: u16, centers: &[u16], area: Rect) {
    if centers.is_empty() || area.height < 2 {
        return;
    }
    let style = Style::new().fg(app.theme.line);
    let buf = f.buffer_mut();
    let lo = centers.iter().copied().min().unwrap().min(boss_x);
    let hi = centers.iter().copied().max().unwrap().max(boss_x);
    buf[(boss_x, area.y)].set_char('│').set_style(style);
    for x in lo..=hi {
        let down = centers.contains(&x);
        let up = x == boss_x;
        let c = match (up, down, x == lo, x == hi) {
            (true, true, _, _) => '┼',
            (true, false, true, _) => '└',
            (true, false, _, true) => '┘',
            (true, false, _, _) => '┴',
            (false, true, true, _) => '┌',
            (false, true, _, true) => '┐',
            (false, true, _, _) => '┬',
            _ => '─',
        };
        buf[(x, area.y + 1)].set_char(c).set_style(style);
    }
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
    let border = match a.status {
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
    if !a.def.effort.is_empty() && right.len() + a.def.effort.len() + 3 <= room {
        right = format!("{right} · {}", a.def.effort);
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

    let detail: Line = if a.def.role == "boss" {
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
    while lines.len() < 4 {
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
