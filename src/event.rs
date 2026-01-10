#[derive(Copy, Clone)]
pub enum PlayerCommand {
    Resume,
    Pause,
}

#[derive(Clone, Copy)]
pub enum AppCommand {
    Err(&'static str),
    End,
}

