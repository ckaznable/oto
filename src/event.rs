use crate::{
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
    MoveQueueCursor(i16),
    PickTrack,
    ImageEncodeResult(ProtocolLruResult),
}

#[derive(Clone)]
pub enum MprisCommand {
    TrackUpdate(TrackMeta, MediaSpec),
    PlayBackStateUpdate(f64, bool),
    VolumeUpdate(u8),
}
