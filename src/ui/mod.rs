mod panels;
mod team;

use crate::app::{App, Focus, Module};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Clear, Paragraph};

pub fn draw(f: &mut Frame, app: &App) {
    let t = &app.theme;
    let area = f.area();
    f.render_widget(Block::new().style(Style::new().bg(t.bg).fg(t.text)), area);

    let [header, body, input, keys] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Fill(1),
        Constraint::Length(3),
        Constraint::Length(1),
    ])
    .areas(area);
    draw_header(f, app, header);

    if app.compact || app.zoom.is_some() {
        let [tabs, content, status] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Fill(1),
            Constraint::Length(1),
        ])
        .areas(body);
        draw_tabs(f, app, tabs);
        draw_module(f, app, app.zoom.unwrap_or(app.tab), content);
        f.render_widget(
            Paragraph::new(panels::status_line(app, status.width)),
            status,
        );
    } else {
        draw_full(f, app, body);
    }

    draw_input(f, app, input);
    f.render_widget(Paragraph::new(key_hints(app)), keys);
    draw_toast(f, app, area);
}

pub fn is_compact(app: &App, w: u16, h: u16) -> bool {
    w < app.cfg.layout.compact_width || h < app.cfg.layout.compact_height
}

fn modules(names: &[String]) -> Vec<Module> {
    names.iter().filter_map(|s| Module::from_name(s)).collect()
}

fn draw_full(f: &mut Frame, app: &App, area: Rect) {
    let l = &app.cfg.layout;
    let (left, right, bottom) = (modules(&l.left), modules(&l.right), modules(&l.bottom));
    let [top, bot] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(if bottom.is_empty() {
            0
        } else {
            l.bottom_height
        }),
    ])
    .areas(area);
    // the left column is a bit narrower than the two right columns together, like the concept
    let [lcol, rcol] = Layout::horizontal([Constraint::Fill(165), Constraint::Fill(200)])
        .spacing(1)
        .areas(top);

    let lc: Vec<Constraint> = left
        .iter()
        .enumerate()
        .map(|(i, m)| {
            if i == 0 {
                Constraint::Fill(1)
            } else {
                Constraint::Length(preferred_height(*m))
            }
        })
        .collect();
    for (m, r) in left
        .iter()
        .zip(Layout::vertical(lc).spacing(1).split(lcol).iter())
    {
        draw_module(f, app, *m, *r);
    }

    let rows = right.len().div_ceil(2).max(1);
    let row_rects = Layout::vertical(vec![Constraint::Fill(1); rows])
        .spacing(1)
        .split(rcol);
    for (i, m) in right.iter().enumerate() {
        let cols = Layout::horizontal([Constraint::Fill(1), Constraint::Fill(1)])
            .spacing(1)
            .split(row_rects[i / 2]);
        // an odd last module takes the whole row
        let r = if i == right.len() - 1 && i % 2 == 0 {
            row_rects[i / 2]
        } else {
            cols[i % 2]
        };
        draw_module(f, app, *m, r);
    }

    if !bottom.is_empty() {
        let bc: Vec<Constraint> = (0..bottom.len())
            .map(|i| {
                if i == 0 {
                    Constraint::Fill(165)
                } else {
                    Constraint::Fill(200 / (bottom.len() as u16 - 1).max(1))
                }
            })
            .collect();
        for (m, r) in bottom
            .iter()
            .zip(Layout::horizontal(bc).spacing(1).split(bot).iter())
        {
            draw_module(f, app, *m, *r);
        }
    }
}

fn preferred_height(m: Module) -> u16 {
    match m {
        Module::Log => 8,
        Module::Usage => 9,
        _ => 12,
    }
}

fn draw_module(f: &mut Frame, app: &App, m: Module, area: Rect) {
    match m {
        Module::Team => team::draw(f, app, area),
        Module::Questions => panels::questions(f, app, area),
        Module::Git => panels::git(f, app, area),
        Module::Web => panels::web(f, app, area),
        Module::Guard => panels::guard(f, app, area),
        Module::Log => panels::log(f, app, area),
        Module::Usage => panels::usage(f, app, area),
    }
}

fn draw_header(f: &mut Frame, app: &App, area: Rect) {
    let t = &app.theme;
    let dim = Style::new().fg(t.dim);
    let br = Style::new().fg(t.bright);
    let mut left = vec![
        Span::styled(" ORDA ", br.add_modifier(Modifier::BOLD)),
        Span::styled(" ", dim),
        Span::styled(crate::app::tilde(&app.cwd), br),
    ];
    if let Some(g) = &app.git {
        left.push(Span::styled("  branch ", dim));
        left.push(Span::styled(g.branch.clone(), br));
        if g.ahead > 0 {
            left.push(Span::styled(
                format!(" +{}", g.ahead),
                Style::new().fg(t.ok),
            ));
        }
    }
    left.push(Span::styled("  task ", dim));
    let task = if app.task.is_empty() {
        "none yet".to_string()
    } else {
        app.task.clone()
    };
    left.push(Span::styled(
        task,
        if app.task.is_empty() { dim } else { br },
    ));

    let mut right = vec![];
    if let Some((done, total)) = app.plan {
        right.push(Span::styled("plan ", dim));
        right.push(Span::styled(format!("{done}/{total} "), br));
        right.extend(bar(done as f64 / total as f64, 9, t.ok, t.line));
    }
    // the owner's task: what it is doing, and how long it has taken (the clock stops when it is done)
    let (state, status) = app.task_state();
    let color = match status {
        crate::app::Status::Done => t.ok,
        crate::app::Status::Running => t.claude,
        crate::app::Status::Asking => t.ask,
        crate::app::Status::Failed => t.bad,
        _ => t.dim,
    };
    right.push(Span::styled("  ", dim));
    right.push(Span::styled(
        format!(" {state} "),
        Style::new()
            .fg(color)
            .bg(t.badge)
            .add_modifier(Modifier::BOLD),
    ));
    if let Some(start) = app.task_started {
        let end = app.task_done.unwrap_or_else(std::time::Instant::now);
        let s = end.duration_since(start).as_secs();
        right.push(Span::styled(
            format!("  {:02}:{:02}:{:02} ", s / 3600, s / 60 % 60, s % 60),
            br,
        ));
    } else {
        right.push(Span::raw(" "));
    }
    f.render_widget(Paragraph::new(lr(left, right, area.width)), area);
}

fn draw_tabs(f: &mut Frame, app: &App, area: Rect) {
    let t = &app.theme;
    let cur = app.zoom.unwrap_or(app.tab);
    let mut spans = vec![Span::raw(" ")];
    for m in app.modules() {
        let style = if m == cur {
            Style::new()
                .fg(t.bright)
                .bg(t.badge)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::new().fg(t.dim)
        };
        spans.push(Span::styled(format!(" {} ", m.name()), style));
        if m == Module::Questions && !app.questions.is_empty() {
            spans.push(Span::styled(
                format!("{} ", app.questions.len()),
                Style::new().fg(t.ask).add_modifier(Modifier::BOLD),
            ));
        }
        spans.push(Span::raw(" "));
    }
    let hint = if app.zoom.is_some() && !app.compact {
        "esc back to all panels "
    } else {
        "tab next "
    };
    f.render_widget(
        Paragraph::new(lr(
            spans,
            vec![Span::styled(hint, Style::new().fg(t.dim))],
            area.width,
        )),
        area,
    );
}

fn draw_input(f: &mut Frame, app: &App, area: Rect) {
    let t = &app.theme;
    let focused = app.focus == Focus::Input;
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(if focused { t.claude } else { t.line }));
    let inner = block.inner(area);
    f.render_widget(block, area);
    let prompt = Span::styled("› ", Style::new().fg(t.claude).add_modifier(Modifier::BOLD));
    let line = if app.input.is_empty() && !focused {
        Line::from(vec![
            prompt,
            Span::styled(
                "press i and tell orda what to build",
                Style::new().fg(t.dim),
            ),
        ])
    } else {
        Line::from(vec![
            prompt,
            Span::styled(app.input.clone(), Style::new().fg(t.bright)),
        ])
    };
    f.render_widget(Paragraph::new(line), inner);
    if focused {
        let x = inner.x + 2 + app.input.chars().count() as u16;
        f.set_cursor_position((x.min(inner.right().saturating_sub(1)), inner.y));
    }
}

fn key_hints(app: &App) -> Line<'static> {
    let t = &app.theme;
    let pairs: &[(&str, &str)] = match app.focus {
        Focus::Input => &[("enter", "send"), ("esc", "back")],
        Focus::Questions => &[
            ("1-9", "pick"),
            ("p", "this project"),
            ("a", "all projects"),
            ("o", "just once"),
            ("j/k", "move"),
            ("esc", "back"),
        ],
        Focus::None => &[
            ("i", "task"),
            ("q", "questions"),
            ("s", "scout"),
            ("t", "team"),
            ("g", "git"),
            ("w", "web"),
            ("x", "guard"),
            ("l", "log"),
            ("u", "usage"),
            ("tab", "next"),
            ("esc", "all panels"),
            ("ctrl+c", "quit"),
        ],
    };
    let mut spans = vec![Span::raw(" ")];
    for (k, v) in pairs {
        spans.push(Span::styled(
            *k,
            Style::new().fg(t.bright).add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(format!(" {v}   "), Style::new().fg(t.dim)));
    }
    Line::from(spans)
}

fn draw_toast(f: &mut Frame, app: &App, area: Rect) {
    let Some((head, text, _)) = &app.toast else {
        return;
    };
    let t = &app.theme;
    let w = 52.min(area.width.saturating_sub(4));
    let r = Rect {
        x: area.right().saturating_sub(w + 2),
        y: area.y + 2,
        width: w,
        height: 5,
    };
    f.render_widget(Clear, r);
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(t.ask))
        .style(Style::new().bg(t.badge));
    let inner = block.inner(r);
    f.render_widget(block, r);
    let lines = vec![
        Line::from(Span::styled(
            format!("? {head}"),
            Style::new().fg(t.ask).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(text.clone(), Style::new().fg(t.bright))),
        Line::from(vec![
            Span::styled("press ", Style::new().fg(t.dim)),
            Span::styled("q", Style::new().fg(t.bright).add_modifier(Modifier::BOLD)),
            Span::styled(" to answer, the team keeps working", Style::new().fg(t.dim)),
        ]),
    ];
    f.render_widget(Paragraph::new(lines), inner);
}

// ---- small shared pieces ----

/// A titled panel in the TUI style: rounded corners, the title cut into the top border.
pub fn panel(
    app: &App,
    title: Vec<Span<'static>>,
    right: String,
    border: Color,
    focused: bool,
) -> Block<'static> {
    let t = &app.theme;
    let mut spans = vec![Span::raw(" ")];
    spans.extend(title);
    spans.push(Span::raw(" "));
    let block = Block::bordered()
        .border_type(if focused {
            BorderType::Thick
        } else {
            BorderType::Rounded
        })
        .border_style(Style::new().fg(border))
        .title_top(Line::from(spans));
    if right.is_empty() {
        return block;
    }
    block.title_top(
        Line::from(Span::styled(format!(" {right} "), Style::new().fg(t.dim))).right_aligned(),
    )
}

/// Left and right spans on one line of `width`, the left side cut short if needed.
pub fn lr(left: Vec<Span<'static>>, right: Vec<Span<'static>>, width: u16) -> Line<'static> {
    let rw: usize = right.iter().map(|s| s.width()).sum();
    let room = (width as usize).saturating_sub(rw + 1);
    let mut out = fit(left, room);
    let lw: usize = out.iter().map(|s| s.width()).sum();
    out.push(Span::raw(
        " ".repeat((width as usize).saturating_sub(lw + rw)),
    ));
    out.extend(right);
    Line::from(out)
}

/// Cut spans to `max` columns, ending with an ellipsis when something was cut.
pub fn fit(spans: Vec<Span<'static>>, max: usize) -> Vec<Span<'static>> {
    let total: usize = spans.iter().map(|s| s.width()).sum();
    if total <= max {
        return spans;
    }
    let mut out = vec![];
    let mut used = 0;
    for s in spans {
        let w = s.width();
        if used + w < max {
            used += w;
            out.push(s);
            continue;
        }
        let keep: String = s
            .content
            .chars()
            .take(max.saturating_sub(used + 1))
            .collect();
        out.push(Span::styled(format!("{keep}…"), s.style));
        break;
    }
    out
}

pub fn bar(frac: f64, width: usize, on: Color, off: Color) -> Vec<Span<'static>> {
    let n = ((frac.clamp(0.0, 1.0) * width as f64).round() as usize).min(width);
    vec![
        Span::styled("█".repeat(n), Style::new().fg(on)),
        Span::styled("░".repeat(width - n), Style::new().fg(off)),
    ]
}

pub fn spark(values: impl Iterator<Item = u64>, width: usize) -> String {
    const LEVELS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    let v: Vec<u64> = values.collect();
    let v = &v[v.len().saturating_sub(width)..];
    let max = v.iter().copied().max().unwrap_or(0).max(1);
    let s: String = v.iter().map(|x| LEVELS[(*x * 7 / max) as usize]).collect();
    format!("{}{s}", "▁".repeat(width - v.len()))
}

pub fn human(n: u64) -> String {
    match n {
        0..1_000 => n.to_string(),
        1_000..1_000_000 => format!("{:.1}k", n as f64 / 1e3),
        _ => format!("{:.2}M", n as f64 / 1e6),
    }
}

pub const SPINNER: [&str; 4] = ["▖", "▘", "▝", "▗"];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helpers() {
        assert_eq!(human(999), "999");
        assert_eq!(human(31_400), "31.4k");
        assert_eq!(spark([0, 7].into_iter(), 4), "▁▁▁█");
        let line = lr(vec![Span::raw("abcdefghij")], vec![Span::raw("xy")], 8);
        assert_eq!(line.width(), 8);
        assert!(line.to_string().contains('…'));
    }
}

#[cfg(test)]
mod render {
    use crate::app::App;
    use ratatui::{Terminal, backend::TestBackend};

    /// Draws the demo at several sizes. `ORDA_PRINT=1 cargo test render -- --nocapture` prints the frames.
    #[test]
    fn demo_renders_at_every_size() {
        for (w, h) in [(196, 54), (150, 40), (80, 24), (40, 12)] {
            let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
            let mut app = App::new(true, tx);
            app.compact = super::is_compact(&app, w, h);
            let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
            term.draw(|f| super::draw(f, &app)).unwrap();
            if std::env::var("ORDA_PRINT").is_ok() {
                let buf = term.backend().buffer();
                for y in 0..h {
                    let row: String = (0..w).map(|x| buf[(x, y)].symbol().to_string()).collect();
                    println!("{row}");
                }
                println!();
            }
        }
    }

    /// Messages caught mid-flight: `ORDA_PRINT=1 cargo test render_flights -- --nocapture`
    #[test]
    fn render_flights() {
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let mut app = App::new(true, tx);
        for (from, to, kind, ago) in [
            ("tester", "builder", "bug", 500),
            ("builder-2", "reviewer", "fixed", 900),
            ("scout", "boss", "escalation", 1000),
        ] {
            let card = crate::roles::Card {
                to: to.into(),
                re: None,
                kind: kind.into(),
                fields: vec![("title".into(), "x".into())],
            };
            app.send(from, card);
            app.flights.last_mut().unwrap().start -= std::time::Duration::from_millis(ago);
        }
        let mut term = Terminal::new(TestBackend::new(196, 54)).unwrap();
        term.draw(|f| super::draw(f, &app)).unwrap();
        let buf = term.backend().buffer();
        let screen: Vec<String> = (0..54)
            .map(|y| (0..90).map(|x| buf[(x, y)].symbol().to_string()).collect())
            .collect();
        assert!(
            screen.iter().any(|l| l.contains("scout -> boss")),
            "the title names the newest flight"
        );
        if std::env::var("ORDA_PRINT").is_ok() {
            println!("{}", screen.join("\n"));
        }
    }

    /// The builder after its codex limit ran out: `ORDA_PRINT=1 cargo test render_fallback -- --nocapture`
    #[test]
    fn render_fallback() {
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let mut app = App::new(true, tx);
        let i = app.agent_index("builder").unwrap();
        app.demo_limit_hit(i, crate::app::now_unix() + 3600);
        let mut term = Terminal::new(TestBackend::new(196, 54)).unwrap();
        term.draw(|f| super::draw(f, &app)).unwrap();
        let buf = term.backend().buffer();
        let screen: Vec<String> = (0..54)
            .map(|y| (0..196).map(|x| buf[(x, y)].symbol().to_string()).collect())
            .collect();
        assert!(
            screen.iter().any(|l| l.contains("opus · fallback")),
            "the card says it is on a fallback"
        );
        if std::env::var("ORDA_PRINT").is_ok() {
            println!("{}", screen.join("\n"));
        }
    }
}
