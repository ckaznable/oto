use crate::{
    event::{CursorMove, PickedPlaylist},
    media::MediaStore,
    tui::state::CursorMovable,
};
use anyhow::anyhow;
use enclose::enclose;
use ratatui_image::picker::Picker;
use std::{
    cell::{Cell, RefCell},
    io::stdout,
    rc::Rc,
    sync::{
        Arc,
        atomic::{self, AtomicU8},
        mpsc::{Receiver, Sender},
    },
    time::{Duration, Instant},
};
use strum::{Display, EnumCount, FromRepr};
use tui_input::backend::crossterm::EventHandler;

use ratatui::{
    DefaultTerminal, Frame,
    crossterm::{
        self,
        event::{Event, KeyCode, KeyEvent, KeyModifiers},
        execute,
        terminal::SetTitle,
    },
    layout::{Constraint, Layout},
    widgets::Block,
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
pub mod scrollbar;
pub mod state;
pub mod state_bar;
pub mod theme;

pub const LAYOUT_WIDTH_S: u16 = 65;
pub const LAYOUT_WIDTH_L: u16 = 120;

#[derive(Clone, Copy, Default, FromRepr)]
#[repr(u8)]
pub enum KeyHandleMode {
    #[default]
    App = 0,
    Edit = 1,
}

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
    cache: Rc<RefCell<state::CacheState>>,
    key_handle_mode: Arc<AtomicU8>,
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
            cache: Rc::new(RefCell::new(state::CacheState::default())),
            key_handle_mode: Arc::new(AtomicU8::new(KeyHandleMode::default() as u8)),
        }
    }

    pub fn key_handle_mode(&self) -> KeyHandleMode {
        KeyHandleMode::from_repr(self.key_handle_mode.load(atomic::Ordering::Relaxed))
            .unwrap_or(KeyHandleMode::App)
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

    let state = AppState::new();

    let key_handle_mode = state.key_handle_mode.clone();
    std::thread::spawn(
        enclose!((tx, player_tx) move || handle_keypress(tx, player_tx, key_handle_mode)),
    );

    ratatui::run(move |t| app(t, tx, rx, player_tx, picker, state))?;
    Ok(())
}

fn app(
    terminal: &mut DefaultTerminal,
    tx: Sender<AppCommand>,
    rx: Receiver<AppCommand>,
    player_tx: Sender<PlayerCommand>,
    picker: Picker,
    state: AppState,
) -> anyhow::Result<()> {
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
            }
            AppCommand::VolumeUpdate(vol) => {
                state.volume.set(vol);
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
                let mut ui_state = state.ui_state.borrow_mut();
                if ui_state.tracks.tree_by_artist.is_empty() {
                    let (by_artist, by_album) = MediaStore::get_tracks_tree(&list.list);
                    ui_state.tracks.tree_by_artist = by_artist;
                    ui_state.tracks.tree_by_album = by_album;
                }

                ui_state.search.filtered_indices = (0..list.list.len()).collect();
                drop(ui_state);

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
                    CollapseWidgets::TrackPicker => {
                        if let Some(len) = ui_state.tracks.len() {
                            ui_state.tracks.move_cursor(steps, len)
                        } else {
                            false
                        }
                    }
                    CollapseWidgets::DevicesList => {
                        let len = state.devices.borrow().len();
                        ui_state.devices.move_cursor(steps, len)
                    }
                    CollapseWidgets::Search => {
                        let len = ui_state.search.filtered_indices.len();
                        ui_state.search.move_cursor(steps, len)
                    }
                    CollapseWidgets::KeyBinding => false,
                };
            }
            AppCommand::MoveCollapseCursor(steps) => {
                force_render = true;

                let mut ui_state = state.ui_state.borrow_mut();
                let index = ui_state.expand_index;

                ui_state.expand_index = if steps > 0 {
                    index + steps as usize
                } else {
                    index.saturating_sub(steps.unsigned_abs() as usize)
                }
                .clamp(0, CollapseWidgets::COUNT - 1)
            }
            AppCommand::MoveSubCollapseCursor(steps) => {
                let mut ui_state = state.ui_state.borrow_mut();
                let index = ui_state.expand_index;

                if matches!(CollapseWidgets::get(index), CollapseWidgets::TrackPicker) {
                    force_render = true;
                    if steps > 0 {
                        ui_state.tracks.expand_index =
                            ui_state.tracks.expand_index.saturating_add(1).clamp(0, 3);
                    } else {
                        ui_state.tracks.expand_index =
                            ui_state.tracks.expand_index.saturating_sub(1).clamp(0, 3);
                    }
                }
            }
            AppCommand::SubmitItem => {
                let mut ui_state = state.ui_state.borrow_mut();
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
                                d = Some((
                                    card.index,
                                    card.devices[card.devices.len().saturating_sub(tmp - index)]
                                        .index,
                                ));
                                break;
                            }
                        }

                        if let Some(d) = d {
                            player_tx.send(PlayerCommand::SetDevice(d)).ok();
                        }
                    }
                    CollapseWidgets::TrackPicker => {
                        if !ui_state.tracks.playlist.is_empty() {
                            let picked = ui_state.tracks.playlist.iter().cloned().collect();
                            player_tx
                                .send(PlayerCommand::SetPickedPlaylist(PickedPlaylist::Picked(
                                    picked,
                                )))
                                .ok();
                            ui_state.tracks.clear_picked();
                        }
                    }
                    CollapseWidgets::Search => {
                        ui_state.search.input.reset();

                        // TODO submit
                    }
                    _ => (),
                };
            }
            AppCommand::AppendItem => {
                let mut ui_state = state.ui_state.borrow_mut();
                if let CollapseWidgets::TrackPicker = CollapseWidgets::get(ui_state.expand_index) {
                    let picked = ui_state.tracks.playlist.iter().cloned().collect();
                    player_tx
                        .send(PlayerCommand::SetPickedPlaylist(PickedPlaylist::Append(
                            picked,
                        )))
                        .ok();
                    ui_state.tracks.clear_picked();
                }
            }
            AppCommand::InsertItem => {
                let mut ui_state = state.ui_state.borrow_mut();
                if let CollapseWidgets::TrackPicker = CollapseWidgets::get(ui_state.expand_index) {
                    let picked = ui_state.tracks.playlist.iter().cloned().collect();
                    player_tx
                        .send(PlayerCommand::SetPickedPlaylist(
                            PickedPlaylist::InsertNext(picked),
                        ))
                        .ok();
                    ui_state.tracks.clear_picked();
                }
            }
            AppCommand::SelectItem => {
                let mut ui_state = state.ui_state.borrow_mut();
                if matches!(
                    CollapseWidgets::get(ui_state.expand_index),
                    CollapseWidgets::TrackPicker
                ) {
                    ui_state.tracks.pick();
                    force_render = true;
                }
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
            AppCommand::TogglePickerMode => {
                use state::TracksMode::*;
                let mut ui_state = state.ui_state.borrow_mut();
                ui_state.tracks.mode = match ui_state.tracks.mode {
                    Artist => Album,
                    Album => Artist,
                };
            }
            AppCommand::Resize => {
                let _ = state
                    .ui_state
                    .borrow_mut()
                    .cover
                    .as_mut()
                    .map(|c| c.reload());
            }
            AppCommand::EditEvent(event) => {
                state
                    .ui_state
                    .borrow_mut()
                    .search
                    .input
                    .handle_event(&event);
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
        let index = state.ui_state.borrow().expand_index;
        let groups: CollapsibleWidgetGroup<{ CollapseWidgets::COUNT }> =
            CollapsibleWidgetGroup::new(CollapseWidgets::widgets(), index, 1);
        frame.render_stateful_widget(groups, main, &mut state);
    }

    frame.render_stateful_widget(MediaInfo, media_info, &mut state);
    frame.render_stateful_widget(StateBar, bottom, &mut state);
}

fn handle_app_keypress(tx: Sender<AppCommand>, player_tx: Sender<PlayerCommand>, event: KeyEvent, mode: Arc<AtomicU8>) {
    match event {
        KeyEvent {
            code: KeyCode::Char('q'),
            ..
        } => {
            tx.send(AppCommand::End).ok();
        }
        KeyEvent {
            code: KeyCode::Char(' '),
            modifiers: KeyModifiers::CONTROL,
            ..
        } => {
            tx.send(AppCommand::SelectItem).ok();
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
            tx.send(AppCommand::MoveListCursor(CursorMove::Steps(1)))
                .ok();
        }
        KeyEvent {
            code: KeyCode::Char('k'),
            ..
        } => {
            tx.send(AppCommand::MoveListCursor(CursorMove::Steps(-1)))
                .ok();
        }
        KeyEvent {
            code: KeyCode::Char('f'),
            modifiers: KeyModifiers::CONTROL,
            ..
        } => {
            tx.send(AppCommand::MoveListCursor(CursorMove::Steps(10)))
                .ok();
        }
        KeyEvent {
            code: KeyCode::Char('b'),
            modifiers: KeyModifiers::CONTROL,
            ..
        } => {
            tx.send(AppCommand::MoveListCursor(CursorMove::Steps(-10)))
                .ok();
        }
        KeyEvent {
            code: KeyCode::Char('d'),
            modifiers: KeyModifiers::CONTROL,
            ..
        } => {
            tx.send(AppCommand::MoveListCursor(CursorMove::Steps(5)))
                .ok();
        }
        KeyEvent {
            code: KeyCode::Char('u'),
            modifiers: KeyModifiers::CONTROL,
            ..
        } => {
            tx.send(AppCommand::MoveListCursor(CursorMove::Steps(-5)))
                .ok();
        }
        KeyEvent {
            code: KeyCode::Char('g'),
            ..
        } => {
            tx.send(AppCommand::MoveListCursor(CursorMove::Start)).ok();
        }
        KeyEvent {
            code: KeyCode::Char('G'),
            ..
        } => {
            tx.send(AppCommand::MoveListCursor(CursorMove::End)).ok();
        }
        KeyEvent {
            code: KeyCode::Char('l'),
            ..
        } => {
            tx.send(AppCommand::MoveSubCollapseCursor(1)).ok();
        }
        KeyEvent {
            code: KeyCode::Char('h'),
            ..
        } => {
            tx.send(AppCommand::MoveSubCollapseCursor(-1)).ok();
        }
        KeyEvent {
            code: KeyCode::Char('L'),
            ..
        } => {
            player_tx.send(PlayerCommand::NextSong).ok();
        }
        KeyEvent {
            code: KeyCode::Char('H'),
            ..
        } => {
            player_tx.send(PlayerCommand::PrevSong).ok();
        }
        KeyEvent {
            code: KeyCode::Enter,
            ..
        } => {
            tx.send(AppCommand::SubmitItem).ok();
        }
        KeyEvent {
            code: KeyCode::Tab, ..
        } => {
            tx.send(AppCommand::SelectItem).ok();
        }
        KeyEvent {
            code: KeyCode::Char('a'),
            modifiers: KeyModifiers::CONTROL,
            ..
        } => {
            tx.send(AppCommand::AppendItem).ok();
        }
        KeyEvent {
            code: KeyCode::Char('i'),
            ..
        } => {
            tx.send(AppCommand::InsertItem).ok();
        }
        KeyEvent {
            code: KeyCode::Char('a'),
            ..
        } => {
            tx.send(AppCommand::TogglePickerMode).ok();
        }
        KeyEvent {
            code: KeyCode::Char('/'),
            ..
        } => {
            mode.store(KeyHandleMode::Edit as u8, atomic::Ordering::Relaxed);
            tx.send(AppCommand::Rerender(true)).ok();
        }
        _ => {}
    }
}

fn handle_edit_keypress(tx: Sender<AppCommand>, event: Event, mode: Arc<AtomicU8>) {
    match event {
        Event::Key(event) => match event {
            KeyEvent {
                code: KeyCode::Esc, ..
            } => {
                mode.store(KeyHandleMode::App as u8, atomic::Ordering::Relaxed);
                tx.send(AppCommand::Rerender(true)).ok();
            }
            KeyEvent {
                code: KeyCode::Enter,
                ..
            } => {
                mode.store(KeyHandleMode::App as u8, atomic::Ordering::Relaxed);
                tx.send(AppCommand::SubmitItem).ok();
            }
            event => {
                tx.send(AppCommand::EditEvent(Event::Key(event))).ok();
            }
        },
        event => {
            tx.send(AppCommand::EditEvent(event)).ok();
        }
    }
}

fn handle_keypress(tx: Sender<AppCommand>, player_tx: Sender<PlayerCommand>, mode: Arc<AtomicU8>) {
    loop {
        let handle_mode = KeyHandleMode::from_repr(mode.load(atomic::Ordering::Relaxed));

        match crossterm::event::read() {
            Ok(Event::Resize(_, _)) => {
                tx.send(AppCommand::Resize).ok();
            }
            Ok(Event::Key(event)) if matches!(handle_mode, Some(KeyHandleMode::App)) => {
                handle_app_keypress(tx.clone(), player_tx.clone(), event, mode.clone());
            }
            Ok(event) => {
                handle_edit_keypress(tx.clone(), event, mode.clone());
            }
            Err(_) => {
                tx.send(AppCommand::Err("crossterm event reader error occurred"))
                    .ok();
            }
        }
    }
}
