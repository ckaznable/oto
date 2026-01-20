use enclose::enclose;
use std::{
    cell::{Cell, RefCell},
    io::stdout,
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
        execute,
        terminal::SetTitle,
    },
    layout::{Constraint, Layout},
    widgets::Block,
};

use crate::{
    event::{AppCommand, PlayerCommand},
    media::{MediaSpec, TrackMeta},
    player::PlayList,
    tui::{
        media_info::MediaInfo, queue_list::QueueList, state::UiState, state_bar::StateBar,
        theme::Theme,
    },
};

pub mod media_info;
pub mod queue_list;
pub mod state;
pub mod state_bar;
pub mod theme;

pub const LAYOUT_WIDTH_S: u16 = 65;
pub const LAYOUT_WIDTH_L: u16 = 120;

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
    ui_state: Rc<RefCell<UiState>>,
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
            ui_state: Rc::new(RefCell::new(UiState::default())),
        }
    }
}

pub fn tui(
    player_tx: Sender<PlayerCommand>,
    tx: Sender<AppCommand>,
    rx: Receiver<AppCommand>,
) -> anyhow::Result<()> {
    std::thread::spawn(enclose!((tx, player_tx) move || handle_keypress(tx, player_tx)));
    ratatui::run(move |t| app(t, rx, player_tx))?;
    Ok(())
}

fn app(
    terminal: &mut DefaultTerminal,
    rx: Receiver<AppCommand>,
    player_tx: Sender<PlayerCommand>,
) -> std::io::Result<()> {
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
                if let Some(ref title) = track.title {
                    execute!(stdout(), SetTitle(format!(" {title}"))).ok();
                }

                *state.playing_track.borrow_mut() = PlayingTrack { track, spec };
                force_render = true;
            }
            AppCommand::PlaylistUpdate(list) => {
                *state.playlist.borrow_mut() = list;
                force_render = true;
            }
            AppCommand::Rerender(force) => {
                force_render = force;
            }
            AppCommand::MoveQueueCursor(steps) => {
                let list_len = state.playlist.borrow().list.len();
                if state
                    .ui_state
                    .borrow_mut()
                    .queue
                    .move_cursor(steps, list_len)
                {
                    force_render = true;
                }
            }
            AppCommand::PickTrack => {
                let index = state.ui_state.borrow().queue.cursor_index;
                player_tx.send(PlayerCommand::PlayTrackWithIndex(index)).ok();
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

    let layout = Layout::horizontal(if area.width > LAYOUT_WIDTH_L {
        [Constraint::Length(50), Constraint::Fill(1)]
    } else if area.width <= LAYOUT_WIDTH_S {
        [Constraint::Fill(1), Constraint::Length(0)]
    } else {
        [Constraint::Length(30), Constraint::Fill(1)]
    });

    let [media_info, queue] = layout.areas(content);

    if area.width > LAYOUT_WIDTH_S {
        frame.render_stateful_widget(QueueList, queue, &mut state);
    }

    frame.render_stateful_widget(MediaInfo, media_info, &mut state);
    frame.render_stateful_widget(StateBar, bottom, &mut state);
}

fn handle_keypress(tx: Sender<AppCommand>, player_tx: Sender<PlayerCommand>) {
    loop {
        match crossterm::event::read() {
            Ok(Event::Resize(_, _)) => {
                tx.send(AppCommand::Rerender(false)).ok();
            }
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
                KeyEvent {
                    code: KeyCode::Char('n'),
                    ..
                } => {
                    tx.send(AppCommand::MoveQueueCursor(1)).ok();
                }
                KeyEvent {
                    code: KeyCode::Char('p'),
                    ..
                } => {
                    tx.send(AppCommand::MoveQueueCursor(-1)).ok();
                }
                KeyEvent {
                    code: KeyCode::Enter,
                    ..
                } => {
                    tx.send(AppCommand::PickTrack).ok();
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
