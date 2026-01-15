use crate::tui::AppMode;

#[derive(Copy, Clone)]
pub enum PlayerCommand {
    PauseCycle,
    SetRelatedVolumn(i8),
}

#[derive(Clone)]
pub enum AppCommand {
    Err(&'static str),
    Unexcepted(String),
    End,
    TimeUpdate(f64, Option<u64>),
    AppModeUpdate(AppMode),
    VolumnUpdate(u8),
}

