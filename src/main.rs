use std::{
    path::PathBuf,
    sync::mpsc::{Receiver, channel},
};

use alsa::pcm::State;
use anyhow::Result;
use clap::Parser;
use oto::{
    cli,
    event::PlayerCommand,
    player::{AudioOutput, BufferPlayer, PlayerError},
};

fn main() -> Result<()> {
    let args = cli::Args::parse();

    let (tx, rx) = channel();

    match args.command {
        cli::Commands::Play { path, device } => {
            player(path, device, rx)
        }
        cli::Commands::PlayList { command } => {
            todo!()
        }
    }
}

fn player(path: impl Into<PathBuf>, device: String, rx: Receiver<PlayerCommand>) -> Result<()> {
    let mut player = BufferPlayer::new(path)?;
    player.init()?;

    let mut output = AudioOutput::new(&device)?;
    output.init(player.spec.unwrap())?;

    loop {
        if let Ok(cmd) = rx.try_recv() {
            match cmd {
                PlayerCommand::Resume => {
                    output.pause(false)?;
                }
                PlayerCommand::Pause => {
                    output.pause(true)?;
                }
            }
        }

        output.wait(Some(32))?;
        if !matches!(output.state(), State::Running | State::Prepared) {
            output.prepare()?;
        }

        if let Err(PlayerError::EOF) = player.consume(&mut output) {
            break;
        }

        if !matches!(output.state(), State::Running | State::Paused) {
            output.start()?;
        }
    }

    output.drain()?;
    Ok(())
}

