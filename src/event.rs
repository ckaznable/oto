use ratatui::crossterm::event::Event;

use crate::{
    devices::PlaybackPCM,
    media::{MediaSpec, TrackMeta},
    player::PlayList,
    tui::{image::ProtocolLruResult, AppMode},
};

#[derive(Clone, Copy)]
pub enum CursorMove {
    Steps(i16),
    Start,
    End,
}

#[derive(Clone, Debug)]
pub enum PickedPlaylist {
    Picked(Vec<usize>),
    InsertNext(Vec<usize>),
    Append(Vec<usize>),
}

#[derive(Clone)]
pub enum PlayerCommand {
    PauseCycle,
    Pause,
    Play,
    SetRelatedVolume(i8),
    SetVolumn(u8),
    NextSong,
    PrevSong,
    PlayTrackWithIndex(usize),
    GetDevices,
    SetDevice((i32, i32)),
    SetPickedPlaylist(PickedPlaylist),
}

pub enum AppCommand {
    Err(&'static str),
    Unexcepted(String),
    End,
    TimeUpdate(f64, Option<u64>),
    AppModeUpdate(AppMode),
    VolumeUpdate(u8),
    TrackUpdate(TrackMeta, MediaSpec),
    PlaylistUpdate(PlayList),
    Rerender(bool),
    Resize,
    MoveListCursor(CursorMove),
    MoveCollapseCursor(i16),
    MoveSubCollapseCursor(i16),
    SubmitItem,
    SelectItem,
    InsertItem,
    AppendItem,
    ImageEncodeResult(ProtocolLruResult),
    DevicesList(Vec<PlaybackPCM>),
    DeviceUpdate((i32, i32)),
    TogglePickerMode,
    EditEvent(Event),
    UpdateFiltered(Vec<usize>),
}

#[derive(Clone)]
pub enum MprisCommand {
    TrackUpdate(TrackMeta, MediaSpec),
    PlayBackStateUpdate(f64, bool),
    VolumeUpdate(u8),
}

#[derive(Clone)]
pub enum MatcherCommand {
    Search(String),
    PlaylistUpdate(PlayList),
}
