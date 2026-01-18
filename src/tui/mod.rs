use enclose::enclose;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::mpsc::{Receiver, Sender}, time::{Duration, Instant},
};
use strum::Display;

use ratatui::{
    DefaultTerminal, Frame,
    crossterm::{
        self,
        event::{Event, KeyCode, KeyEvent},
    },
    layout::{Constraint, Layout},
};

use crate::{
    event::{AppCommand, PlayerCommand}, media::{MediaSpec, TrackMeta}, player::PlayList, tui::{media_info::MediaInfo, queue_list::QueueList, state_bar::StateBar}
};

pub mod media_info;
pub mod queue_list;
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

#[derive(Default)]
pub struct PlayingTrack {
    track: TrackMeta,
    spec: MediaSpec,
}

#[derive(Clone, Default)]
pub struct AppState {
    app_mode: Rc<Cell<AppMode>>,
    play_mode: Rc<Cell<PlayMode>>,
    playing: Rc<Cell<PlayingState>>,
    playing_track: Rc<RefCell<PlayingTrack>>,
    playlist: Rc<RefCell<PlayList>>,
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

    let min_refresh_duration = Duration::from_secs_f64(1. / 60.);
    let mut timer = Instant::now();
    let mut should_render = true;

    loop {
        if should_render {
            timer = Instant::now();
            terminal.draw(enclose!((state) move |f| render(f, state)))?;
        }

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
                AppCommand::TrackUpdate(track, spec) => {
                    log::info!("playing: {:?}", &track);
                    *state.playing_track.borrow_mut() = PlayingTrack { track, spec };
                }
                AppCommand::PlaylistUpdate(list) => {
                    *state.playlist.borrow_mut() = list;
                }
            }
        }

        let refresh_time = timer.elapsed();
        should_render = refresh_time > min_refresh_duration;
    }
}

fn render(frame: &mut Frame, mut state: AppState) {
    let area = frame.area();
    let layout = Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]);

    let [content, bottom] = layout.areas(area);

    let layout = Layout::horizontal([Constraint::Length(50), Constraint::Fill(1)]);

    let [media_info, queue] = layout.areas(content);

    frame.render_stateful_widget(MediaInfo, media_info, &mut state);
    frame.render_stateful_widget(QueueList, queue, &mut state);
    frame.render_stateful_widget(StateBar, bottom, &mut state);
}

fn handle_keypress(tx: Sender<AppCommand>, player_tx: Sender<PlayerCommand>) {
    loop {
        match crossterm::event::read() {
            Ok(Event::Key(event)) => match event {
                KeyEvent {
                    code: KeyCode::Char('q'),
                    ..
                } => {
                    tx.send(AppCommand::End).ok();
                }
                KeyEvent {
                    code: KeyCode::Char(' '),
                    ..
                } => {
                    player_tx.send(PlayerCommand::PauseCycle).ok();
                }
                KeyEvent {
                    code: KeyCode::Char('j'),
                    ..
                } => {
                    player_tx.send(PlayerCommand::SetRelatedVolume(-5)).ok();
                }
                KeyEvent {
                    code: KeyCode::Char('k'),
                    ..
                } => {
                    player_tx.send(PlayerCommand::SetRelatedVolume(5)).ok();
                }
                KeyEvent {
                    code: KeyCode::Char('l'),
                    ..
                } => {
                    player_tx.send(PlayerCommand::NextSong).ok();
                }
                KeyEvent {
                    code: KeyCode::Char('h'),
                    ..
                } => {
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
