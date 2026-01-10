use enclose::enclose;
use std::sync::mpsc::{Receiver, Sender};

use ratatui::{DefaultTerminal, Frame, crossterm};

use crate::event::{AppCommand, PlayerCommand};

#[derive(Clone)]
struct AppStatus {}

impl AppStatus {
    fn new() -> Self {
        Self {}
    }
}

pub fn tui(
    player_tx: Sender<PlayerCommand>,
    tx: Sender<AppCommand>,
    rx: Receiver<AppCommand>,
) -> anyhow::Result<()> {
    std::thread::spawn(enclose!((tx) move || handle_keypress(tx)));
    ratatui::run(move |t| app(t, rx))?;
    Ok(())
}

fn app(terminal: &mut DefaultTerminal, rx: Receiver<AppCommand>) -> std::io::Result<()> {
    let status = AppStatus::new();

    loop {
        terminal.draw(enclose!((status) move |f| render(f, status)))?;
        match rx.recv() {
            Err(_) => break Ok(()),
            Ok(event) => match event {
                AppCommand::Err(e) => log::error!("{e}"),
                AppCommand::End => break Ok(()),
            },
        }
    }
}

fn render(frame: &mut Frame, status: AppStatus) {
    frame.render_widget("hello world", frame.area());
}

fn handle_keypress(tx: Sender<AppCommand>) {
    loop {
        match crossterm::event::read() {
            Ok(_) => {}
            Err(e) => {
                tx.send(AppCommand::Err("crossterm event reader error occurred"))
                    .ok();
            }
        }
    }
}
