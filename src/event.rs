use crate::{
    devices::PlaybackPCM,
    media::{MediaSpec, TrackMeta},
    player::PlayList,
    tui::{AppMode, image::ProtocolLruResult},
};

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
    MoveListCursor(i16),
    MoveCollapseCursor(i16),
    MoveSubCollapseCursor(i16),
    SelectItem,
    ImageEncodeResult(ProtocolLruResult),
    DevicesList(Vec<PlaybackPCM>),
    DeviceUpdate((i32, i32)),
}

#[derive(Clone)]
pub enum MprisCommand {
    TrackUpdate(TrackMeta, MediaSpec),
    PlayBackStateUpdate(f64, bool),
    VolumeUpdate(u8),
}
