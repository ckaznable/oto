use crate::media::MediaSpec;

#[derive(Copy, Clone)]
pub enum PlayerCommand {
    Play(MediaSpec),
    Resume,
    Pause,
}

