use enclose::enclose;
use strum::Display;
use std::{
    cell::Cell,
    rc::Rc,
    sync::mpsc::{Receiver, Sender},
};

use ratatui::{
    DefaultTerminal, Frame, crossterm::{self, event::Event},
    layout::{Constraint, Layout},
};

use crate::{event::{AppCommand, PlayerCommand}, tui::state_bar::StateBar};

pub mod state_bar;

#[derive(Clone, Copy, Default, Display)]
pub enum AppMode {
    #[default]
    Normal,
    Playing,
    Pause,
}

#[derive(Clone, Copy, Default)]
pub enum PlayMode {
    #[default]
    Normal,
    Loop,
    LoopCurrent,
}

#[derive(Clone, Copy, Default)]
pub struct PlayingState {
    current: usize,
    duration: usize,
}

#[derive(Clone, Default)]
pub struct AppState {
    app_mode: Rc<Cell<AppMode>>,
    play_mode: Rc<Cell<PlayMode>>,
    playing: Rc<Cell<PlayingState>>,
    volumn: Rc<Cell<u8>>,
}

impl AppState {
    fn new() -> Self {
        Self {
            ..Default::default()
        }
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
    let status = AppState::new();

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

fn render(frame: &mut Frame, mut state: AppState) {
    let area = frame.area();
    let layout = Layout::vertical([
        Constraint::Length(1),
        Constraint::Fill(1),
        Constraint::Length(1),
    ]);

    let [top, middle, bottom] = layout.areas(area);
    frame.render_stateful_widget(StateBar, bottom, &mut state);
}

fn handle_keypress(tx: Sender<AppCommand>) {
    loop {
        match crossterm::event::read() {
            Ok(Event::Key(_)) => {
                tx.send(AppCommand::End).ok();
            },
            Err(e) => {
                tx.send(AppCommand::Err("crossterm event reader error occurred"))
                    .ok();
            }
            _ => {}
        }
    }
}
