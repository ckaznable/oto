use anyhow::anyhow;
use enclose::enclose;
use ratatui_image::picker::Picker;
use std::{
    cell::{Cell, RefCell},
    io::stdout,
    rc::Rc,
    sync::mpsc::{Receiver, Sender},
    time::{Duration, Instant},
};
use strum::{Display, EnumCount};

use ratatui::{
    crossterm::{
        self,
        event::{Event, KeyCode, KeyEvent, KeyModifiers},
        execute,
        terminal::SetTitle,
    },
    layout::{Constraint, Layout},
    widgets::Block,
    DefaultTerminal, Frame,
};

use crate::{
    devices::PlaybackPCM,
    event::{AppCommand, PlayerCommand},
    media::{MediaSpec, TrackMeta},
    player::PlayList,
    tui::{
        collapsible::{CollapseWidgets, CollapsibleWidgetGroup},
        image::LruProtocolFactory,
        media_info::MediaInfo,
        state::UiState,
        state_bar::StateBar,
        theme::Theme,
    },
};

pub mod collapsible;
pub mod image;
pub mod media_info;
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

#[derive(Clone, Default)]
pub struct DevicesState {
    list: Vec<PlaybackPCM>,
    current: (i32, i32),
}

impl DevicesState {
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.list.iter().map(|p| p.devices.len()).sum()
    }
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
    devices: Rc<RefCell<DevicesState>>,
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
            devices: Rc::new(RefCell::new(Default::default())),
        }
    }
}

pub fn tui(
    player_tx: Sender<PlayerCommand>,
    tx: Sender<AppCommand>,
    rx: Receiver<AppCommand>,
) -> anyhow::Result<()> {
    let picker = match Picker::from_query_stdio() {
        Ok(p) => p,
        Err(e) => {
            log::error!("{e:?}");
            return Err(anyhow!("{e:?}"));
        }
    };

    log::info!("image protocol {picker:?}");

    std::thread::spawn(enclose!((tx, player_tx) move || handle_keypress(tx, player_tx)));
    ratatui::run(move |t| app(t, tx, rx, player_tx, picker))?;
    Ok(())
}

fn app(
    terminal: &mut DefaultTerminal,
    tx: Sender<AppCommand>,
    rx: Receiver<AppCommand>,
    player_tx: Sender<PlayerCommand>,
    picker: Picker,
) -> anyhow::Result<()> {
    let state = AppState::new();

    let mut lru_protocol_factory = LruProtocolFactory::new()?;
    lru_protocol_factory.on_cached(move |result| {
        log::debug!("got image encode result");
        tx.send(AppCommand::ImageEncodeResult(result)).ok();
    });
    lru_protocol_factory.spawn(picker)?;

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

                let mut playing = state.playing_track.borrow_mut();
                let mut ui_state = state.ui_state.borrow_mut();
                let track_path = track.path();

                if !playing.track.is_album_same(&track) {
                    log::debug!("update cover image");
                    let protocol = lru_protocol_factory.new_uncached_protocol(track_path);
                    ui_state.cover.replace(protocol);
                }

                *playing = PlayingTrack { track, spec };
                force_render = true;
            }
            AppCommand::PlaylistUpdate(list) => {
                *state.playlist.borrow_mut() = list;
                force_render = true;
            }
            AppCommand::Rerender(force) => {
                force_render = force;
            }
            AppCommand::MoveListCursor(steps) => {
                let index = state.ui_state.borrow().expand_index;
                let mut ui_state = state.ui_state.borrow_mut();

                force_render = match CollapseWidgets::get(index) {
                    CollapseWidgets::QueueList => {
                        let len = state.playlist.borrow().list.len();
                        ui_state.queue.move_cursor(steps, len)
                    }
                    CollapseWidgets::DevicesList => {
                        let len = state.devices.borrow().len();
                        ui_state.devices.move_cursor(steps, len)
                    }
                    CollapseWidgets::KeyBinding => false,
                };
            }
            AppCommand::MoveCollapseCursor(steps) => {
                let mut ui_state = state.ui_state.borrow_mut();
                let index = ui_state.expand_index;

                ui_state.expand_index = if steps > 0 {
                    index + steps as usize
                } else {
                    index.saturating_sub(steps.unsigned_abs() as usize)
                }
                .clamp(0, CollapseWidgets::COUNT - 1)
            }
            AppCommand::SelectItem => {
                let ui_state = state.ui_state.borrow();
                match CollapseWidgets::get(ui_state.expand_index) {
                    CollapseWidgets::QueueList => {
                        let index = ui_state.queue.cursor_index;
                        player_tx
                            .send(PlayerCommand::PlayTrackWithIndex(index))
                            .ok();
                    }
                    CollapseWidgets::DevicesList => {
                        let index = ui_state.devices.cursor_index;
                        let list = &state.devices.borrow().list;
                        let mut tmp = 0usize;
                        let mut d = None;
                        for card in list {
                            tmp += card.devices.len();
                            if tmp > index {
                                d = Some((card.index, card.devices[card.devices.len().saturating_sub(tmp - index)].index));
                                break;
                            }
                        }

                        if let Some(d) = d {
                            player_tx.send(PlayerCommand::SetDevice(d)).ok();
                        }
                    }
                    _ => (),
                };
            }
            AppCommand::ImageEncodeResult(result) => {
                force_render = true;
                match result {
                    image::ProtocolLruResult::Cached(_) => todo!(),
                    image::ProtocolLruResult::UnCached(_, stateful_protocol) => {
                        let _ = state
                            .ui_state
                            .borrow_mut()
                            .cover
                            .as_mut()
                            .map(|p| p.update_resized_protocol(stateful_protocol));
                    }
                }
            }
            AppCommand::DevicesList(devices) => {
                log::info!("devices: {devices:?}");
                let mut d = state.devices.borrow_mut();
                d.list = devices;
            }
            AppCommand::DeviceUpdate(device) => {
                log::info!("current device: {device:?}");
                let mut d = state.devices.borrow_mut();
                d.current = device;
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

    let [media_info, main] = layout.areas(content);

    if area.width > LAYOUT_WIDTH_S {
        frame.render_stateful_widget(CollapsibleWidgetGroup, main, &mut state);
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
                    code: KeyCode::Char('J'),
                    ..
                } => {
                    player_tx.send(PlayerCommand::SetRelatedVolume(-5)).ok();
                }
                KeyEvent {
                    code: KeyCode::Char('K'),
                    ..
                } => {
                    player_tx.send(PlayerCommand::SetRelatedVolume(5)).ok();
                }
                KeyEvent {
                    code: KeyCode::Char('j'),
                    modifiers: KeyModifiers::CONTROL,
                    ..
                } => {
                    tx.send(AppCommand::MoveCollapseCursor(1)).ok();
                }
                KeyEvent {
                    code: KeyCode::Char('k'),
                    modifiers: KeyModifiers::CONTROL,
                    ..
                } => {
                    tx.send(AppCommand::MoveCollapseCursor(-1)).ok();
                }
                KeyEvent {
                    code: KeyCode::Char('j'),
                    ..
                } => {
                    tx.send(AppCommand::MoveListCursor(1)).ok();
                }
                KeyEvent {
                    code: KeyCode::Char('k'),
                    ..
                } => {
                    tx.send(AppCommand::MoveListCursor(-1)).ok();
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
                    code: KeyCode::Enter,
                    ..
                } => {
                    tx.send(AppCommand::SelectItem).ok();
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
