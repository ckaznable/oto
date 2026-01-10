use log::LevelFilter;
use simplelog::WriteLogger;
use std::{
    path::PathBuf,
    sync::mpsc::{Receiver, Sender, channel},
};

use alsa::pcm::State;
use anyhow::Result;
use clap::Parser;
use oto::{
    cli,
    event::{AppCommand, PlayerCommand},
    player::{AudioOutput, BufferPlayer, PlayerError},
};

fn main() -> Result<()> {
    let args = cli::Args::parse();

    let (player_tx, player_rx) = channel();
    let (app_tx, app_rx) = channel();

    match args.command {
        cli::Commands::Play { path, device } => {
            player_tx.send(PlayerCommand::Resume).ok();
            player(path, device, app_tx, player_rx)
        }
        cli::Commands::Tui { path, device } => {
            WriteLogger::init(
                    LevelFilter::Info,
                    simplelog::Config::default(),
                    std::fs::File::create("/tmp/oto.log").unwrap(),
                ).unwrap();

            // use enclose::enclose;
            // std::thread::spawn(enclose!((app_tx) move || player(path, device, app_tx, player_rx)));

            oto::tui::tui(player_tx, app_tx, app_rx)
        }
    }
}

fn player(path: impl Into<PathBuf>, device: String, tx: Sender<AppCommand>, rx: Receiver<PlayerCommand>) -> Result<()> {
    let mut player = BufferPlayer::new(path)?;
    player.init()?;

    let mut output = AudioOutput::new(&device)?;
    output.init(player.spec.unwrap())?;

    loop {
        if let Ok(cmd) = rx.try_recv() {
            match cmd {
                PlayerCommand::Resume => {
                    if !matches!(output.state(), State::Running | State::Paused) {
                        output.start()?;
                    }

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
    }

    output.drain()?;
    Ok(())
}

