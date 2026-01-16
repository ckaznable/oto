use crate::{media::TrackMeta, tui::AppMode};

#[derive(Clone)]
pub enum PlayerCommand {
    PauseCycle,
    Pause,
    Play,
    SetRelatedVolume(i8),
    NextSong,
    PrevSong,
}

#[derive(Clone)]
pub enum AppCommand {
    Err(&'static str),
    Unexcepted(String),
    End,
    TimeUpdate(f64, Option<u64>),
    AppModeUpdate(AppMode),
    VolumeUpdate(u8),
    TrackUpdate(TrackMeta),
}

#[derive(Clone)]
pub enum MprisCommand {
    TrackUpdate(TrackMeta),
}
