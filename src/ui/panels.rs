use super::{SPINNER, bar, fit, human, lr, panel, spark};
use crate::app::{App, Focus, Scope, now_unix};
use crate::git::Tone;
use crate::guard::Verdict;
use crate::limits::until;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

fn bold(c: ratatui::style::Color) -> Style {
    Style::new().fg(c).add_modifier(Modifier::BOLD)
}

/// Render lines inside a block, one column of padding on each side.
fn body(f: &mut Frame, area: Rect, lines: Vec<Line<'static>>) {
    let r = Rect {
        x: area.x + 1,
        width: area.width.saturating_sub(2),
        ..area
    };
    let lines: Vec<Line> = lines
        .into_iter()
        .take(r.height as usize)
        .map(|l| Line::from(fit(l.spans, r.width as usize)))
        .collect();
    f.render_widget(Paragraph::new(lines), r);
}

pub fn questions(f: &mut Frame, app: &App, area: Rect) {
    let t = &app.theme;
    let n = app.questions.len();
    let focused = app.focus == Focus::Questions;
    // waiting questions pulse so they get noticed
    let border = if n > 0 && (focused || (app.frame / 8).is_multiple_of(2)) {
        t.ask
    } else {
        t.line
    };
    let count = if n == 0 {
        "  none waiting".to_string()
    } else {
        format!("  {n} new")
    };
    let right = if focused {
        "answering".to_string()
    } else if n > 0 {
        "q to answer".into()
    } else {
        "work never waits".into()
    };
    let block = panel(
        app,
        vec![
            Span::styled("questions", bold(t.ask)),
            Span::styled(count, Style::new().fg(t.dim)),
        ],
        right,
        border,
        focused,
    );
    let inner = block.inner(area);
    f.render_widget(block, area);

    let mut lines: Vec<Line> = vec![];
    for (i, q) in app.questions.iter().enumerate() {
        let sel = i == app.q_sel;
        let mark = if sel && focused { "▌" } else { " " };
        let ago = q.asked.elapsed().as_secs() / 60;
        lines.push(Line::from(vec![
            Span::styled(mark, Style::new().fg(t.ask)),
            Span::styled(q.from.clone(), Style::new().fg(app.accent_of(&q.from))),
            Span::styled(
                format!(
                    " · {} · {}",
                    q.task,
                    if ago == 0 {
                        "now".into()
                    } else {
                        format!("{ago}m ago")
                    }
                ),
                Style::new().fg(t.dim),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::styled(mark, Style::new().fg(t.ask)),
            Span::styled(q.text.clone(), Style::new().fg(t.bright)),
        ]));
        if sel {
            let mut opts = vec![Span::raw(" ")];
            for (k, o) in q.options.iter().enumerate() {
                let chosen = q.chosen == Some(k);
                opts.push(Span::styled(format!("{}", k + 1), Style::new().fg(t.dim)));
                opts.push(Span::styled(
                    format!(" {o} "),
                    if chosen {
                        Style::new().fg(t.ask).bg(t.badge)
                    } else {
                        Style::new().fg(t.text).bg(t.badge)
                    },
                ));
                opts.push(Span::raw("  "));
            }
            lines.push(Line::from(opts));
            if q.chosen.is_some() && q.guard_cmd.is_none() {
                let k = |c: &str| Span::styled(c.to_string(), bold(t.bright));
                let d = |c: &str| Span::styled(c.to_string(), Style::new().fg(t.dim));
                lines.push(Line::from(vec![
                    d(" remember for  "),
                    k("p"),
                    d(" this project  "),
                    k("a"),
                    d(" all projects  "),
                    k("o"),
                    d(" just once"),
                ]));
            }
        }
        lines.push(Line::default());
    }
    let room = (inner.height as usize).saturating_sub(lines.len() + 1);
    if room > 0 && !app.remembered.is_empty() {
        lines.push(Line::from(Span::styled(
            " remembered",
            Style::new().fg(t.dim),
        )));
        for r in app.remembered.iter().take(room.saturating_sub(1)) {
            let (tag, c) = match r.scope {
                Scope::All => ("all", t.fable),
                Scope::Project => ("project", t.codex),
                Scope::Once => ("once", t.dim),
            };
            lines.push(Line::from(vec![
                Span::raw(" "),
                Span::styled(format!("{tag:<8}"), Style::new().fg(c)),
                Span::styled(r.text.clone(), Style::new().fg(t.text)),
            ]));
        }
    }
    body(f, inner, lines);
}

pub fn git(f: &mut Frame, app: &App, area: Rect) {
    let t = &app.theme;
    let Some(g) = &app.git else {
        let block = panel(
            app,
            vec![Span::styled("git", bold(t.bright))],
            "not a repo".into(),
            t.line,
            false,
        );
        let inner = block.inner(area);
        f.render_widget(block, area);
        body(
            f,
            inner,
            vec![Line::from(Span::styled(
                "orda is not running inside a git repo",
                Style::new().fg(t.dim),
            ))],
        );
        return;
    };
    let ahead = if g.ahead > 0 {
        format!(" +{}", g.ahead)
    } else {
        String::new()
    };
    let right = format!(
        "{}{ahead} · {} worktree{}",
        g.branch,
        g.worktrees.len(),
        if g.worktrees.len() == 1 { "" } else { "s" }
    );
    let block = panel(
        app,
        vec![Span::styled("git", bold(t.bright))],
        right,
        t.line,
        false,
    );
    let inner = block.inner(area);
    f.render_widget(block, area);
    let w = inner.width.saturating_sub(2);
    let dim = Style::new().fg(t.dim);

    let mut lines = vec![Line::from(Span::styled("worktrees", dim))];
    for wt in &g.worktrees {
        let c = app
            .agents
            .iter()
            .find(|a| wt.name.ends_with(&a.def.name))
            .map(|a| t.accent(&a.def.vendor, &a.def.model))
            .unwrap_or(t.text);
        lines.push(Line::from(vec![
            Span::styled(wt.name.clone(), Style::new().fg(c)),
            Span::styled(format!("  {} files ", wt.files), dim),
            Span::styled(format!("+{}", wt.added), Style::new().fg(t.ok)),
            Span::styled(format!(" -{}", wt.removed), Style::new().fg(t.bad)),
        ]));
    }
    lines.push(Line::from(Span::styled("commits", dim)));
    for (i, c) in g.commits.iter().enumerate() {
        let held = app
            .holds
            .iter()
            .find(|h| h.0.starts_with(&c.hash) || c.hash.starts_with(&h.0));
        let status = held
            .map(|h| (format!("held by {}", h.2), Tone::Bad))
            .or(c.status.clone());
        let (label, color) = match &status {
            Some((s, tone)) => (
                s.clone(),
                match tone {
                    Tone::Busy => t.ask,
                    Tone::Good => t.ok,
                    Tone::Bad => t.bad,
                    Tone::Info => t.codex,
                },
            ),
            None => (c.when.clone(), t.dim),
        };
        let mut line = lr(
            vec![
                Span::styled(c.hash.clone(), Style::new().fg(t.ask)),
                Span::styled(
                    format!(" {}", c.subject),
                    Style::new().fg(if i == 0 { t.bright } else { t.text }),
                ),
            ],
            vec![Span::styled(label, Style::new().fg(color))],
            w,
        );
        if i == 0 {
            line = line.style(Style::new().bg(t.badge));
        }
        lines.push(line);
    }
    let room = (inner.height as usize).saturating_sub(lines.len());
    if room >= 2 && !g.head_files.is_empty() {
        lines.push(Line::from(Span::styled(
            format!(
                "{} files",
                g.commits.first().map(|c| c.hash.as_str()).unwrap_or("HEAD")
            ),
            dim,
        )));
        for (st, path, a, r) in &g.head_files {
            lines.push(Line::from(vec![
                Span::styled(format!("{st:<2}{path}  "), dim),
                Span::styled(format!("+{a}"), Style::new().fg(t.ok)),
                Span::styled(format!(" -{r}"), Style::new().fg(t.bad)),
            ]));
        }
    }
    body(f, inner, lines);
}

pub fn web(f: &mut Frame, app: &App, area: Rect) {
    let t = &app.theme;
    let block = panel(
        app,
        vec![
            Span::styled("web", bold(t.gemini)),
            Span::styled(
                format!("  {} searches", app.web.len()),
                Style::new().fg(t.dim),
            ),
        ],
        "what agents looked up".into(),
        t.line,
        false,
    );
    let inner = block.inner(area);
    f.render_widget(block, area);
    let mut lines = vec![];
    if app.web.is_empty() {
        lines.push(Line::from(Span::styled(
            "no searches yet",
            Style::new().fg(t.dim),
        )));
    }
    for w in &app.web {
        let accent = app.accent_of(&w.who);
        let mut head = vec![
            Span::styled(w.who.clone(), Style::new().fg(accent)),
            Span::raw(" "),
        ];
        if w.live {
            head.push(Span::styled(
                format!("{} ", SPINNER[app.frame % 4]),
                Style::new().fg(accent),
            ));
        }
        head.push(Span::styled(w.query.clone(), Style::new().fg(t.bright)));
        lines.push(Line::from(head));
        for s in &w.sources {
            lines.push(Line::from(Span::styled(
                format!("  {s}"),
                Style::new().fg(t.dim),
            )));
        }
        if let Some(l) = &w.learned {
            lines.push(Line::from(Span::styled(
                format!("  learned: {l}"),
                Style::new().fg(t.ok),
            )));
        }
    }
    body(f, inner, lines);
}

pub fn guard(f: &mut Frame, app: &App, area: Rect) {
    let t = &app.theme;
    let g = &app.cfg.guard;
    let blocked = app
        .guard_events
        .iter()
        .filter(|e| e.verdict == Verdict::Deny)
        .count();
    let block = panel(
        app,
        vec![
            Span::styled("guard", bold(t.bad)),
            Span::styled(
                format!("  {} rules", g.deny.len() + g.ask.len()),
                Style::new().fg(t.dim),
            ),
        ],
        format!("{blocked} blocked"),
        t.line,
        false,
    );
    let inner = block.inner(area);
    f.render_widget(block, area);
    let w = inner.width.saturating_sub(2) as usize;

    // rule chips, packed into lines, one group per verdict
    let mut lines: Vec<Line> = vec![];
    for (label, rules, color) in [("blocked", &g.deny, t.bad), ("asks you", &g.ask, t.ask)] {
        let mut cur = vec![Span::styled(format!("{label:<9}"), Style::new().fg(t.dim))];
        let mut used = 9;
        for rule in rules {
            let chip = format!(" {rule} ");
            let cw = chip.chars().count() + 1;
            if used + cw > w {
                lines.push(Line::from(std::mem::replace(
                    &mut cur,
                    vec![Span::raw(" ".repeat(9))],
                )));
                used = 9;
            }
            cur.push(Span::styled(chip, Style::new().fg(color).bg(t.badge)));
            cur.push(Span::raw(" "));
            used += cw;
        }
        lines.push(Line::from(cur));
    }
    lines.push(Line::default());
    for e in &app.guard_events {
        let (label, c) = match e.verdict {
            Verdict::Deny => ("blocked", t.bad),
            Verdict::Ask => ("asked you", t.ask),
            Verdict::Allow => ("allowed", t.ok),
        };
        lines.push(Line::from(vec![
            Span::styled(format!("{} ", e.at), Style::new().fg(t.dim)),
            Span::styled(e.who.clone(), Style::new().fg(app.accent_of(&e.who))),
            Span::styled(format!(" {label} "), Style::new().fg(c)),
            Span::styled(e.cmd.clone(), Style::new().fg(t.bright)),
        ]));
        lines.push(Line::from(Span::styled(
            format!("      {}", e.why),
            Style::new().fg(t.dim),
        )));
    }
    body(f, inner, lines);
}

pub fn log(f: &mut Frame, app: &App, area: Rect) {
    let t = &app.theme;
    let block = panel(
        app,
        vec![Span::styled("session log", bold(t.bright))],
        "l to expand".into(),
        t.line,
        false,
    );
    let inner = block.inner(area);
    f.render_widget(block, area);
    // long entries wrap under the message column; the newest lines stay at the bottom
    const INDENT: usize = 9 + 11;
    let room = (inner.width as usize).saturating_sub(2 + INDENT).max(10);
    let mut lines: Vec<Line> = vec![];
    for l in app
        .log
        .iter()
        .skip(app.log.len().saturating_sub(inner.height as usize))
    {
        for (k, chunk) in wrap(&l.text, room).into_iter().enumerate() {
            let head = if k == 0 {
                vec![
                    Span::styled(format!("{} ", l.at), Style::new().fg(t.dim)),
                    Span::styled(
                        format!("{:<11}", l.who),
                        Style::new().fg(app.accent_of(&l.who)),
                    ),
                ]
            } else {
                vec![Span::raw(" ".repeat(INDENT))]
            };
            lines.push(Line::from(
                [head, vec![Span::styled(chunk, Style::new().fg(t.text))]].concat(),
            ));
        }
    }
    let lines = lines.split_off(lines.len().saturating_sub(inner.height as usize));
    body(f, inner, lines);
}

pub fn usage(f: &mut Frame, app: &App, area: Rect) {
    let t = &app.theme;
    let block = panel(
        app,
        vec![Span::styled("limits & tokens", bold(t.bright))],
        "this session".into(),
        t.line,
        false,
    );
    let inner = block.inner(area);
    f.render_widget(block, area);
    let inner = Rect {
        x: inner.x + 1,
        width: inner.width.saturating_sub(2),
        ..inner
    };
    // side by side when there is room, limits above tokens when not
    let [left, right] = if inner.width >= 80 {
        Layout::horizontal([Constraint::Fill(11), Constraint::Fill(10)])
            .spacing(3)
            .areas(inner)
    } else {
        let rows = app
            .limits
            .iter()
            .map(|s| s.windows.len().max(1))
            .sum::<usize>()
            .max(1) as u16;
        Layout::vertical([Constraint::Length(rows), Constraint::Fill(1)])
            .spacing(1)
            .areas(inner)
    };
    let now = now_unix();

    let mut lim = vec![];
    if app.limits.is_empty() {
        lim.push(Line::from(Span::styled(
            "limit-watch is not running",
            Style::new().fg(t.dim),
        )));
    }
    let bw = (left.width as usize)
        .saturating_sub(7 + 4 + 5 + 13)
        .clamp(4, 16);
    for s in &app.limits {
        let c = t.accent(&s.vendor, "");
        if let Some(e) = &s.error {
            let why = if e == "auth" {
                "login expired, open it once"
            } else {
                "unavailable"
            };
            lim.push(Line::from(vec![
                Span::styled(format!("{:<7}", s.vendor), Style::new().fg(c)),
                Span::styled(why, Style::new().fg(t.bad)),
            ]));
            continue;
        }
        if let Some(until) = app.blocked.get(&s.vendor).filter(|t| **t > now) {
            lim.push(Line::from(vec![
                Span::styled(format!("{:<7}", s.vendor), Style::new().fg(c)),
                Span::styled(
                    format!("out of limits, back at {}", crate::app::hhmm(*until)),
                    Style::new().fg(t.bad),
                ),
            ]));
            continue;
        }
        for (i, w) in s.windows.iter().enumerate() {
            let name = if i == 0 {
                s.vendor.clone()
            } else {
                String::new()
            };
            let mut spans = vec![
                Span::styled(format!("{name:<7}"), Style::new().fg(c)),
                Span::styled(format!("{:<6}", w.label), Style::new().fg(t.dim)),
            ];
            let hot = if w.used_pct >= 90.0 {
                t.bad
            } else if w.used_pct >= 70.0 {
                t.ask
            } else {
                c
            };
            spans.extend(bar(w.used_pct / 100.0, bw, hot, t.line));
            spans.push(Span::styled(
                format!(" {:>3.0}%", w.used_pct),
                Style::new().fg(t.bright),
            ));
            spans.push(Span::styled(
                format!("  resets {}", until(w.resets_at, now)),
                Style::new().fg(t.dim),
            ));
            lim.push(Line::from(spans));
        }
    }
    f.render_widget(Paragraph::new(lim), left);

    let mut tok = vec![];
    let mut total = 0;
    for vendor in ["claude", "codex", "gemini"] {
        let sum: u64 = app.agents.iter().map(|a| a.tokens_on(vendor)).sum();
        if !app.agents.iter().any(|a| a.def.vendor == vendor) {
            continue;
        }
        total += sum;
        let c = t.accent(vendor, "");
        let sw = (right.width as usize).saturating_sub(7 + 9).clamp(4, 24);
        let hist = app
            .spark
            .get(vendor)
            .map(|h| h.iter().copied().collect::<Vec<_>>())
            .unwrap_or_default();
        tok.push(lr(
            vec![
                Span::styled(format!("{vendor:<7}"), Style::new().fg(c)),
                Span::styled(spark(hist.into_iter(), sw), Style::new().fg(c)),
            ],
            vec![Span::styled(human(sum), Style::new().fg(t.bright))],
            right.width,
        ));
    }
    tok.push(Line::default());
    tok.push(lr(
        vec![Span::styled("total", Style::new().fg(t.dim))],
        vec![Span::styled(human(total), bold(t.bright))],
        right.width,
    ));
    f.render_widget(Paragraph::new(tok), right);
}

/// One line of limits and tokens, shown under every tab in compact mode.
pub fn status_line(app: &App, width: u16) -> Line<'static> {
    let t = &app.theme;
    let mut spans = vec![Span::raw(" ")];
    for s in &app.limits {
        let c = t.accent(&s.vendor, "");
        spans.push(Span::styled(s.vendor.clone(), Style::new().fg(c)));
        if s.error.is_some() {
            spans.push(Span::styled(" login expired", Style::new().fg(t.bad)));
        }
        if let Some(until) = app.blocked.get(&s.vendor).filter(|u| **u > now_unix()) {
            spans.push(Span::styled(
                format!(" out until {}   ", crate::app::hhmm(*until)),
                Style::new().fg(t.bad),
            ));
            continue;
        }
        for w in s.windows.iter() {
            spans.push(Span::styled(
                format!(" {} ", w.label),
                Style::new().fg(t.dim),
            ));
            spans.push(Span::styled(
                format!("{:.0}%", w.used_pct),
                Style::new().fg(t.bright),
            ));
        }
        spans.push(Span::styled("   ", Style::new()));
    }
    let total: u64 = app.agents.iter().map(|a| a.tokens()).sum();
    lr(
        spans,
        vec![
            Span::styled("tokens ", Style::new().fg(t.dim)),
            Span::styled(format!("{} ", human(total)), bold(t.bright)),
        ],
        width,
    )
}

/// Split text into lines of at most `width` characters, breaking at spaces when it can.
fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut out = vec![];
    let mut cur = String::new();
    for word in text.split(' ') {
        let len = cur.chars().count();
        if len > 0 && len + 1 + word.chars().count() > width {
            out.push(std::mem::take(&mut cur));
        }
        if !cur.is_empty() {
            cur.push(' ');
        }
        cur.push_str(word);
        while cur.chars().count() > width {
            let head: String = cur.chars().take(width).collect();
            cur = cur.chars().skip(width).collect();
            out.push(head);
        }
    }
    out.push(cur);
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn wrap_breaks_at_spaces() {
        assert_eq!(
            super::wrap("switched gpt-6.1-sol -> opus (claude)", 20),
            ["switched gpt-6.1-sol", "-> opus (claude)"]
        );
        assert_eq!(super::wrap("abcdefghij", 4), ["abcd", "efgh", "ij"]);
        assert_eq!(super::wrap("short", 20), ["short"]);
    }
}
