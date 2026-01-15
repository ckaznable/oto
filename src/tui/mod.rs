use enclose::enclose;
use strum::Display;
use std::{
    cell::Cell,
    rc::Rc,
    sync::mpsc::{Receiver, Sender},
};

use ratatui::{
    DefaultTerminal, Frame, crossterm::{self, event::{Event, KeyCode, KeyEvent}},
    layout::{Constraint, Layout},
};

use crate::{event::{AppCommand, PlayerCommand}, tui::state_bar::StateBar};

pub mod state_bar;

#[derive(Clone, Copy, Default, Display)]
pub enum AppMode {
    #[default]
    Normal,
    Playing,
    Paused,
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
    current: f64,
    duration: u64,
}

#[derive(Clone, Default)]
pub struct AppState {
    app_mode: Rc<Cell<AppMode>>,
    play_mode: Rc<Cell<PlayMode>>,
    playing: Rc<Cell<PlayingState>>,
    volume: Rc<Cell<u8>>,
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
    std::thread::spawn(enclose!((tx) move || handle_keypress(tx, player_tx)));
    ratatui::run(move |t| app(t, rx))?;
    Ok(())
}

fn app(terminal: &mut DefaultTerminal, rx: Receiver<AppCommand>) -> std::io::Result<()> {
    let state = AppState::new();

    loop {
        terminal.draw(enclose!((state) move |f| render(f, state)))?;

        match rx.recv() {
            Err(_) => break Ok(()),
            Ok(event) => match event {
                AppCommand::Err(e) => log::error!("{e}"),
                AppCommand::Unexcepted(e) => log::error!("{e}"),
                AppCommand::End => break Ok(()),
                AppCommand::TimeUpdate(current, duration) => {
                    state.app_mode.set(AppMode::Playing);
                    state.playing.set(PlayingState {
                        current,
                        duration: duration.unwrap_or(0),
                    });
                }
                AppCommand::AppModeUpdate(mode) => {
                    state.app_mode.set(mode);
                }
                AppCommand::VolumeUpdate(vol) => {
                    state.volume.set(vol);
                }
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

fn handle_keypress(tx: Sender<AppCommand>, player_tx: Sender<PlayerCommand>,) {
    loop {
        match crossterm::event::read() {
            Ok(Event::Key(event)) => match event {
                KeyEvent { code: KeyCode::Char('q'), .. } => {
                    tx.send(AppCommand::End).ok();
                }
                KeyEvent { code: KeyCode::Char(' '), .. } => {
                    player_tx.send(PlayerCommand::PauseCycle).ok();
                }
                KeyEvent { code: KeyCode::Char('j'), .. } => {
                    player_tx.send(PlayerCommand::SetRelatedVolume(-5)).ok();
                }
                KeyEvent { code: KeyCode::Char('k'), .. } => {
                    player_tx.send(PlayerCommand::SetRelatedVolume(5)).ok();
                }
                KeyEvent { code: KeyCode::Char('l'), .. } => {
                    player_tx.send(PlayerCommand::NextSong).ok();
                }
                KeyEvent { code: KeyCode::Char('h'), .. } => {
                    player_tx.send(PlayerCommand::PrevSong).ok();
                }
                _ => {}
            },
            Err(_) => {
                tx.send(AppCommand::Err("crossterm event reader error occurred"))
                    .ok();
            }
            _ => {}
        }
    }
}
