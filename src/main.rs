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
            spawn_mock_app_event_handler(app_rx);
            player(path, device, app_tx, player_rx)
        }
        cli::Commands::Tui { path, device } => {
            WriteLogger::init(
                LevelFilter::Info,
                simplelog::Config::default(),
                std::fs::File::create("/tmp/oto.log").unwrap(),
            )
            .unwrap();

            use enclose::enclose;
            std::thread::spawn(enclose!((app_tx) move || {
                if let Err(e) = player(path, device, app_tx.clone(), player_rx) {
                    app_tx.send(AppCommand::Unexcepted(e.to_string())).ok();
                    log::error!("{e:?}");
                }
            }));

            oto::tui::tui(player_tx, app_tx, app_rx)
        }
    }
}

fn spawn_mock_app_event_handler(rx: Receiver<AppCommand>) {
    std::thread::spawn(move || {
        loop {
            let _ = rx.recv();
        }
    });
}

fn player(
    path: impl Into<PathBuf>,
    device: String,
    tx: Sender<AppCommand>,
    rx: Receiver<PlayerCommand>,
) -> Result<()> {
    let mut player = BufferPlayer::new(path)?;
    player.init()?;

    let mut output = AudioOutput::new(&device)?;
    output.init(player.spec.unwrap())?;

    loop {
        if let Ok(cmd) = rx.try_recv() {
            match cmd {
                PlayerCommand::PauseCycle => {
                    let state = output.state();

                    let pause = matches!(state, State::Running);
                    output.pause(pause)?;

                    tx.send(AppCommand::AppModeUpdate(match pause {
                        true => oto::tui::AppMode::Pause,
                        false => oto::tui::AppMode::Playing,
                    }))
                    .ok();
                }
            }
        }

        output.wait(Some(32))?;
        if !matches!(output.state(), State::Running | State::Prepared) {
            output.prepare()?;
        }

        tx.send(AppCommand::TimeUpdate(
            player.calc_duration(),
            player.spec.and_then(|s| s.duration),
        ))
        .ok();

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
