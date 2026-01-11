use crate::tui::AppMode;

#[derive(Copy, Clone)]
pub enum PlayerCommand {
    PauseCycle,
}

#[derive(Clone)]
pub enum AppCommand {
    Err(&'static str),
    Unexcepted(String),
    End,
    TimeUpdate(f64, Option<u64>),
    AppModeUpdate(AppMode),
}

