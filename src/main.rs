mod agents;
mod app;
mod config;
mod demo;
mod git;
mod guard;
mod limits;
mod theme;
mod ui;

use app::{App, Msg};
use ratatui::crossterm::event::{self, Event, KeyEventKind};
use std::time::Duration;

const HELP: &str = "orda: one boss agent, a team of Claude, Codex and Gemini workers

usage:
  orda           open the dashboard in this directory
  orda --demo    a scripted team, no agents run, no tokens spent
  orda config    write the default settings file and print its path";

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let arg = std::env::args().nth(1);
    match arg.as_deref() {
        None | Some("--demo") => {}
        Some("config") => return config::write_default(),
        Some(_) => {
            println!("{HELP}");
            return Ok(());
        }
    }

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let mut app = App::new(arg.is_some(), tx.clone());

    let mut terminal = ratatui::init();
    // keys come from a plain thread: crossterm's reader blocks
    std::thread::spawn(move || {
        while let Ok(ev) = event::read() {
            let msg = match ev {
                Event::Key(k) if k.kind == KeyEventKind::Press => Msg::Key(k),
                Event::Resize(..) => Msg::Redraw,
                _ => continue,
            };
            if tx.send(msg).is_err() {
                break;
            }
        }
    });

    let mut tick = tokio::time::interval(Duration::from_millis(140));
    let mut dirty = true;
    let result = loop {
        if dirty {
            let size = terminal.size()?;
            app.compact = ui::is_compact(&app, size.width, size.height);
            if let Err(e) = terminal.draw(|f| ui::draw(f, &app)) {
                break Err(e);
            }
        }
        tokio::select! {
            Some(msg) = rx.recv() => {
                app.handle(msg);
                dirty = true;
            }
            _ = tick.tick() => dirty = app.tick(),
        }
        if app.quit {
            break Ok(());
        }
    };
    ratatui::restore();
    result
}
