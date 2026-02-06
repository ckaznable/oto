use crate::{
    alloc::StringArena,
    event::{CursorMove, MatcherCommand, PickedPlaylist},
    media::MediaStore,
    tui::state::{CacheState, CursorMovable},
};
use anyhow::anyhow;
use enclose::enclose;
use itertools::Either;
use nucleo_matcher::{
    Matcher,
    pattern::{AtomKind, CaseMatching, Normalization, Pattern},
};
use ratatui_image::picker::Picker;
use std::{
    cell::{Cell, RefCell},
    io::stdout,
    rc::Rc,
    sync::{
        Arc,
        atomic::{self, AtomicU8},
        mpsc::{Receiver, Sender, channel},
    },
    time::{Duration, Instant},
};
use strum::EnumCount;
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
    event::{AppCommand, PlayerCommand},
    player::PlayList,
    tui::{
        collapsible::{CollapseWidgets, CollapsibleWidgetGroup},
        image::LruProtocolFactory,
        media_info::MediaInfo,
        state::{
            AppMode, DevicesState, KeyHandleMode, PlayMode, PlayingState, PlayingTrack,
            PreRenderState, UiState,
        },
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

#[derive(Clone)]
pub struct AppState {
    alloc: Rc<RefCell<StringArena>>,
    app_mode: Rc<Cell<AppMode>>,
    play_mode: Rc<Cell<PlayMode>>,
    playing: Rc<Cell<PlayingState>>,
    playing_track: Rc<RefCell<PlayingTrack>>,
    playlist: Rc<RefCell<PlayList>>,
    volume: Rc<Cell<u8>>,
    theme: Rc<Theme>,
    ui_state: Rc<RefCell<UiState>>,
    devices: Rc<RefCell<DevicesState>>,
    cache: Rc<RefCell<CacheState>>,
    key_handle_mode: Arc<AtomicU8>,
    pre_render: Rc<RefCell<PreRenderState>>,
}

impl AppState {
    fn new() -> Self {
        let theme = Rc::new(Theme::default());
        let keybinding_lines = crate::tui::collapsible::keybinding::build_keybinding_lines(&theme);

        Self {
            theme,
            alloc: Rc::new(RefCell::new(StringArena::new())),
            app_mode: Rc::new(Cell::new(AppMode::default())),
            play_mode: Rc::new(Cell::new(PlayMode::default())),
            playing: Rc::new(Cell::new(PlayingState::default())),
            playing_track: Rc::new(RefCell::new(PlayingTrack::default())),
            playlist: Rc::new(RefCell::new(PlayList::default())),
            volume: Rc::new(Cell::new(0)),
            ui_state: Rc::new(RefCell::new(UiState::default())),
            devices: Rc::new(RefCell::new(Default::default())),
            cache: Rc::new(RefCell::new(state::CacheState::default())),
            key_handle_mode: Arc::new(AtomicU8::new(KeyHandleMode::default() as u8)),
            pre_render: Rc::new(RefCell::new(PreRenderState {
                keybinding_lines,
                ..Default::default()
            })),
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
    std::thread::Builder::new()
        .name("handle-crossterm-event".into())
        .spawn(enclose!((tx, player_tx) move || handle_keypress(tx, player_tx, key_handle_mode)))?;

    let (matcher_tx, matcher_rx) = channel();
    std::thread::Builder::new()
        .name("fuzzy-mathcer".into())
        .spawn(enclose!((tx, matcher_tx) move || matcher(matcher_tx, tx, matcher_rx)))?;

    ratatui::run(move |t| app(t, tx, rx, player_tx, matcher_tx, picker, state))?;
    Ok(())
}

fn app(
    terminal: &mut DefaultTerminal,
    tx: Sender<AppCommand>,
    rx: Receiver<AppCommand>,
    player_tx: Sender<PlayerCommand>,
    matcher_tx: Sender<MatcherCommand>,
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
                let mut ui_state = state.ui_state.borrow_mut();
                if ui_state.tracks.tree_by_artist.is_empty() {
                    let (by_artist, by_album) = MediaStore::get_tracks_tree(&list.list);
                    ui_state.tracks.tree_by_artist = by_artist;
                    ui_state.tracks.tree_by_album = by_album;
                }

                let playlist = state.playlist.borrow();
                if playlist.list.len() < list.list.len() {
                    ui_state.search.filtered_indices = (0..list.list.len()).collect();
                    matcher_tx
                        .send(MatcherCommand::PlaylistUpdate(list.clone()))
                        .ok();
                }

                if playlist.picked.as_deref().map_or(0, |p| p.len())
                    != list.picked.as_deref().map_or(0, |p| p.len())
                {
                    ui_state.queue.move_to_start();
                }
                drop(ui_state);
                drop(playlist);

                *state.playlist.borrow_mut() = list;
                force_render = true;
            }
            AppCommand::Rerender(force) => {
                force_render = force;
            }
            AppCommand::MoveListCursor(steps) => {
                let index = state.ui_state.borrow().expand_index;
                let mut ui_state = state.ui_state.borrow_mut();

                let len = match CollapseWidgets::get(index) {
                    CollapseWidgets::QueueList => Some(state.playlist.borrow().list.len()),
                    CollapseWidgets::TrackPicker => ui_state.tracks.len(),
                    CollapseWidgets::DevicesList => Some(state.devices.borrow().len()),
                    CollapseWidgets::Search => Some(ui_state.search.filtered_indices.len()),
                    CollapseWidgets::KeyBinding => None,
                };

                force_render = len.is_some_and(|len| ui_state.move_cursor(steps, len));
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
                let ui_state = state.ui_state.borrow_mut();
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
                    CollapseWidgets::TrackPicker | CollapseWidgets::Search => {
                        let playlist = ui_state.picked_playlist.borrow();
                        if !playlist.is_empty() {
                            let picked = playlist.iter().cloned().collect();
                            drop(playlist);
                            player_tx
                                .send(PlayerCommand::SetPickedPlaylist(PickedPlaylist::Picked(
                                    picked,
                                )))
                                .ok();
                            ui_state.picked_playlist.borrow_mut().clear();
                        }
                    }
                    _ => (),
                };
            }
            AppCommand::AppendItem => {
                let ui_state = state.ui_state.borrow();
                if matches!(
                    CollapseWidgets::get(ui_state.expand_index),
                    CollapseWidgets::TrackPicker | CollapseWidgets::Search
                ) {
                    let picked = ui_state.picked_playlist.borrow().iter().cloned().collect();
                    player_tx
                        .send(PlayerCommand::SetPickedPlaylist(PickedPlaylist::Append(
                            picked,
                        )))
                        .ok();
                    ui_state.picked_playlist.borrow_mut().clear();
                }
            }
            AppCommand::InsertItem => {
                let ui_state = state.ui_state.borrow();
                if matches!(
                    CollapseWidgets::get(ui_state.expand_index),
                    CollapseWidgets::TrackPicker | CollapseWidgets::Search
                ) {
                    let picked = ui_state.picked_playlist.borrow().iter().cloned().collect();
                    player_tx
                        .send(PlayerCommand::SetPickedPlaylist(
                            PickedPlaylist::InsertNext(picked),
                        ))
                        .ok();
                    ui_state.picked_playlist.borrow_mut().clear();
                }
            }
            AppCommand::SelectItem => {
                let mut ui_state = state.ui_state.borrow_mut();
                match CollapseWidgets::get(ui_state.expand_index) {
                    CollapseWidgets::TrackPicker => {
                        ui_state.tracks.pick();
                        force_render = true;
                    }
                    CollapseWidgets::Search => {
                        let cursor = ui_state.search.cursor_index;
                        if let Some(&track_idx) = ui_state.search.filtered_indices.get(cursor) {
                            let mut playlist = ui_state.picked_playlist.borrow_mut();
                            if playlist.contains(&track_idx) {
                                playlist.shift_remove(&track_idx);
                            } else {
                                playlist.insert(track_idx);
                            }
                        }
                        force_render = true;
                    }
                    _ => (),
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
                let mut mut_device_state = state.devices.borrow_mut();
                mut_device_state.list = devices;
                drop(mut_device_state);

                let devices_state = state.devices.borrow();
                let theme = &state.theme;
                let lines = crate::tui::collapsible::devices_list::build_devices_lines(
                    &devices_state,
                    theme,
                );
                drop(devices_state);

                state.pre_render.borrow_mut().devices_lines = lines;
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
                let mut ui_state = state.ui_state.borrow_mut();
                ui_state.search.input.handle_event(&event);
                matcher_tx
                    .send(MatcherCommand::Search(
                        ui_state.search.input.value().to_owned(),
                        None,
                    ))
                    .ok();
            }
            AppCommand::UpdateFiltered(query, indices) => {
                let mut ui_state = state.ui_state.borrow_mut();
                if matches!(
                    CollapseWidgets::get(ui_state.expand_index),
                    CollapseWidgets::Search
                ) && query == ui_state.search.input.value()
                {
                    ui_state.search.filtered_indices = indices;
                    force_render = true;
                }
            }
        }

        if force_render || timer.elapsed() >= min_refresh_duration {
            terminal.draw(enclose!((state) move |f| render(f, state)))?;
            timer = Instant::now();
        }
    }
}

struct MatcherItem<'a> {
    search: &'a str,
    index: usize,
}

impl<'a> AsRef<str> for MatcherItem<'a> {
    fn as_ref(&self) -> &str {
        self.search
    }
}

fn matcher(_self_tx: Sender<MatcherCommand>, tx: Sender<AppCommand>, rx: Receiver<MatcherCommand>) {
    let mut playlist: Vec<String> = vec![];

    let mut config = nucleo_matcher::Config::DEFAULT;
    config.ignore_case = true;
    let mut matcher = Matcher::new(config);

    loop {
        match rx.recv() {
            Err(_) => break,
            Ok(MatcherCommand::Search(query, limited)) => {
                let iter = match limited {
                    Some(ref limited) => Either::Left(
                        limited
                            .iter()
                            .filter_map(|i| Some((*i, playlist.get(*i)?)))
                            .map(|(index, search)| MatcherItem { index, search }),
                    ),
                    None => Either::Right(
                        playlist
                            .iter()
                            .enumerate()
                            .map(|(index, search)| MatcherItem { index, search }),
                    ),
                };

                let list: Vec<usize> = Pattern::new(
                    &query,
                    CaseMatching::Ignore,
                    Normalization::Smart,
                    AtomKind::Fuzzy,
                )
                .match_list(iter, &mut matcher)
                .iter()
                .map(|(m, _)| m.index)
                .collect();

                tx.send(AppCommand::UpdateFiltered(query, list)).ok();
            }
            Ok(MatcherCommand::PlaylistUpdate(list)) => {
                #[cfg(feature = "dict-jp")]
                std::thread::spawn(
                    enclose!((_self_tx, list) move || crate::dict::kanji_to_romaji(_self_tx, &list.list)),
                );

                playlist = list
                    .list
                    .iter()
                    .cloned()
                    .map(|track| {
                        format!(
                            "{}-{}-{}",
                            track.title.unwrap_or_default(),
                            track.artist.unwrap_or_default(),
                            track.album.name.unwrap_or_default(),
                        )
                    })
                    .collect();
            }
            Ok(MatcherCommand::KanjiToRomaji(romaji)) => {
                playlist.iter_mut().enumerate().for_each(|(i, s)| {
                    if let Some(Some(r)) = romaji.get(i)
                        && !r.is_empty()
                    {
                        s.insert(0, ' ');
                        s.insert_str(0, r);
                    }
                });

                unsafe {
                    libc::malloc_trim(0);
                }
            }
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

fn handle_app_keypress(
    tx: Sender<AppCommand>,
    player_tx: Sender<PlayerCommand>,
    event: KeyEvent,
    mode: Arc<AtomicU8>,
) {
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
                code: KeyCode::Esc | KeyCode::Enter,
                ..
            } => {
                mode.store(KeyHandleMode::App as u8, atomic::Ordering::Relaxed);
                tx.send(AppCommand::Rerender(true)).ok();
            }
            KeyEvent {
                code: KeyCode::Char('n'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                tx.send(AppCommand::MoveListCursor(CursorMove::Steps(1)))
                    .ok();
            }
            KeyEvent {
                code: KeyCode::Char('p'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                tx.send(AppCommand::MoveListCursor(CursorMove::Steps(-1)))
                    .ok();
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
