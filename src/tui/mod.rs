use enclose::enclose;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::mpsc::{Receiver, Sender},
    time::{Duration, Instant},
};
use strum::Display;

use ratatui::{
    DefaultTerminal, Frame,
    crossterm::{
        self,
        event::{Event, KeyCode, KeyEvent},
    },
    layout::{Constraint, Layout},
    widgets::Block,
};

use crate::{
    event::{AppCommand, PlayerCommand},
    media::{MediaSpec, TrackMeta},
    player::PlayList,
    tui::{media_info::MediaInfo, queue_list::QueueList, state_bar::StateBar, theme::Theme},
};

pub mod media_info;
pub mod queue_list;
pub mod state_bar;
pub mod theme;

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

#[derive(Clone)]
pub struct AppState {
    app_mode: Rc<Cell<AppMode>>,
    play_mode: Rc<Cell<PlayMode>>,
    playing: Rc<Cell<PlayingState>>,
    playing_track: Rc<RefCell<PlayingTrack>>,
    playlist: Rc<RefCell<PlayList>>,
    volume: Rc<Cell<u8>>,
    theme: Rc<Theme>,
}

impl AppState {
    fn new() -> Self {
        Self {
            app_mode: Rc::new(Cell::new(AppMode::default())),
            play_mode: Rc::new(Cell::new(PlayMode::default())),
            playing: Rc::new(Cell::new(PlayingState::default())),
            playing_track: Rc::new(RefCell::new(PlayingTrack::default())),
            playlist: Rc::new(RefCell::new(PlayList::default())),
            volume: Rc::new(Cell::new(0)),
            theme: Rc::new(Theme::default()),
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

    let min_refresh_duration = Duration::from_secs_f64(1. / 30.);
    let mut timer = Instant::now();

    // first draw
    terminal.draw(enclose!((state) move |f| render(f, state)))?;

    loop {
        let event = match rx.recv() {
            Ok(e) => e,
            Err(_) => break Ok(()),
        };

        let mut force_render = false;

        match event {
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
                force_render = true;
            }
            AppCommand::VolumeUpdate(vol) => {
                state.volume.set(vol);
                force_render = true;
            }
            AppCommand::TrackUpdate(track, spec) => {
                log::info!("playing: {:?}", &track);
                *state.playing_track.borrow_mut() = PlayingTrack { track, spec };
                force_render = true;
            }
            AppCommand::PlaylistUpdate(list) => {
                *state.playlist.borrow_mut() = list;
                force_render = true;
            }
        }

        if force_render || timer.elapsed() >= min_refresh_duration {
            terminal.draw(enclose!((state) move |f| render(f, state)))?;
            timer = Instant::now();
        }
    }
}

fn render(frame: &mut Frame, mut state: AppState) {
    let area = frame.area();

    frame.render_widget(
        Block::default().style(ratatui::style::Style::default().bg(state.theme.base)),
        area,
    );

    let layout = Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]);

    let [content, bottom] = layout.areas(area);

    let layout = Layout::horizontal([Constraint::Length(50), Constraint::Fill(1)]);

    let [media_info, queue] = layout.areas(content);

    frame.render_stateful_widget(MediaInfo::default(), media_info, &mut state);
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
