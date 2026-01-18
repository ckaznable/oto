use gag::Redirect;
use log::LevelFilter;
use os_pipe::{PipeWriter, pipe};
use simplelog::WriteLogger;
use std::{
    io::{BufRead, BufReader},
    path::PathBuf,
    sync::mpsc::{Receiver, Sender, channel},
};

use alsa::pcm::State;
use anyhow::Result;
use clap::Parser;
use oto::{
    cli,
    event::{AppCommand, MprisCommand, PlayerCommand},
    mpris,
    player::{AudioOutput, BufferPlayer, LastPlayerState, PlayerError},
    volume::VolumeController,
};

fn main() -> Result<()> {
    let args = cli::Args::parse();

    let (player_tx, player_rx) = channel();
    let (mpris_tx, mpris_rx) = channel();
    let (app_tx, app_rx) = channel();

    let _mpris = mpris::Mpris::handle(player_tx.clone(), mpris_rx)?;

    match args.command {
        cli::Commands::Play { path, device } => {
            spawn_mock_app_event_handler(app_rx);
            player_event_loop(path, device, app_tx, mpris_tx, player_rx)
        }
        cli::Commands::Tui { path, device } => {
            WriteLogger::init(
                LevelFilter::Info,
                simplelog::Config::default(),
                std::fs::File::create("/tmp/oto.log").unwrap(),
            )
            .unwrap();

            let _guard = redirect_stderr_to_log();

            use enclose::enclose;
            std::thread::spawn(enclose!((app_tx) move || {
                if let Err(e) = player_event_loop(path, device, app_tx.clone(), mpris_tx, player_rx) {
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

fn redirect_stderr_to_log() -> Redirect<PipeWriter> {
    let (reader, writer) = pipe().unwrap();

    let redirect = Redirect::stderr(writer).unwrap();

    std::thread::spawn(move || {
        let mut buf_reader = BufReader::new(reader);
        let mut line = String::new();

        while let Ok(len) = buf_reader.read_line(&mut line) {
            if len == 0 {
                break;
            }

            let clean_line = line.trim();
            if !clean_line.is_empty() {
                log::error!(target: "stderr", "{}", clean_line);
            }
            line.clear();
        }
    });

    redirect
}

fn player_event_loop(
    path: impl Into<PathBuf>,
    device: String,
    tx: Sender<AppCommand>,
    mtx: Sender<MprisCommand>,
    rx: Receiver<PlayerCommand>,
) -> Result<()> {
    let mut player = BufferPlayer::new(path)?;
    player.init()?;

    let init_spec = player.spec.unwrap();

    let mut output = AudioOutput::new(&device)?;
    output.init(init_spec)?;

    let vc = VolumeController::new(&device);
    let mut volume = vc.get_volume().unwrap_or(0);

    tx.send(AppCommand::VolumeUpdate(volume as u8)).ok();
    mtx.send(MprisCommand::VolumeUpdate(volume as u8)).ok();

    let mut current_time = 0.;
    if let Some(track) = player.current() {
        tx.send(AppCommand::TrackUpdate(track.clone(), init_spec))
            .ok();
        tx.send(AppCommand::PlaylistUpdate(player.playlist.clone()))
            .ok();
        mtx.send(MprisCommand::TrackUpdate(track, init_spec)).ok();
        mtx.send(MprisCommand::PlayBackStateUpdate(current_time, true))
            .ok();
    }

    loop {
        if let Ok(cmd) = rx.try_recv() {
            match cmd {
                PlayerCommand::Play => {
                    if matches!(output.state(), State::Suspended) {
                        output.resume()?;
                    }

                    output.pause(false)?;
                    mtx.send(MprisCommand::PlayBackStateUpdate(current_time, true))
                        .ok();
                    tx.send(AppCommand::AppModeUpdate(oto::tui::AppMode::Playing))
                        .ok();
                }
                PlayerCommand::Pause => {
                    if matches!(output.state(), State::Suspended) {
                        output.resume()?;
                    }

                    output.pause(true)?;
                    mtx.send(MprisCommand::PlayBackStateUpdate(current_time, false))
                        .ok();
                    tx.send(AppCommand::AppModeUpdate(oto::tui::AppMode::Paused))
                        .ok();
                }
                PlayerCommand::PauseCycle => {
                    if matches!(output.state(), State::Suspended) {
                        output.resume()?;
                    }

                    let pause = !matches!(output.state(), State::Paused);
                    output.pause(pause)?;

                    mtx.send(MprisCommand::PlayBackStateUpdate(current_time, !pause))
                        .ok();
                    tx.send(AppCommand::AppModeUpdate(match pause {
                        true => oto::tui::AppMode::Paused,
                        false => oto::tui::AppMode::Playing,
                    }))
                    .ok();
                }
                PlayerCommand::SetRelatedVolume(vol) => {
                    let v = (volume + vol as i64).clamp(0, 100);
                    if vc.set_volume(v).is_ok() {
                        tx.send(AppCommand::VolumeUpdate(v as u8)).ok();
                        mtx.send(MprisCommand::VolumeUpdate(v as u8)).ok();
                        volume = v;
                    }
                }
                PlayerCommand::SetVolumn(vol) => {
                    if vc.set_volume(vol as i64).is_ok() {
                        tx.send(AppCommand::VolumeUpdate(vol)).ok();
                        mtx.send(MprisCommand::VolumeUpdate(vol)).ok();
                        volume = vol as i64;
                    }
                }
                PlayerCommand::NextSong => {
                    if let Ok(Some(track)) = player.next() {
                        player.clear_buffer();
                        output.drop().ok();
                        output.prepare().ok();

                        let spec = player.spec.unwrap_or_default();
                        tx.send(AppCommand::TrackUpdate(track.clone(), spec)).ok();
                        tx.send(AppCommand::PlaylistUpdate(player.playlist.clone())).ok();
                        mtx.send(MprisCommand::TrackUpdate(track, spec)).ok();
                    }
                }
                PlayerCommand::PrevSong => {
                    if let Ok(Some(track)) = player.prev() {
                        player.clear_buffer();
                        output.drop().ok();
                        output.prepare().ok();

                        let spec = player.spec.unwrap_or_default();
                        tx.send(AppCommand::TrackUpdate(track.clone(), spec)).ok();
                        tx.send(AppCommand::PlaylistUpdate(player.playlist.clone())).ok();
                        mtx.send(MprisCommand::TrackUpdate(track, spec)).ok();
                    }
                }
            }
        }

        if matches!(output.state(), State::Paused) {
            std::thread::sleep(std::time::Duration::from_millis(100));
            continue;
        }

        output.wait(Some(32))?;
        if !matches!(
            output.state(),
            State::Running | State::Prepared | State::Paused
        ) {
            output.prepare()?;
        }

        current_time = player.calc_duration(match output.delay() {
            Ok(d) => d as u64,
            Err(_) => 0,
        });
        tx.send(AppCommand::TimeUpdate(
            current_time,
            player.spec.and_then(|s| s.duration),
        ))
        .ok();

        match player.pop_state() {
            Some(LastPlayerState::PlayListChanged) => {
                if let Some(track) = player.current() {
                    tx.send(AppCommand::TrackUpdate(track, player.spec.unwrap_or_default())).ok();
                    tx.send(AppCommand::PlaylistUpdate(player.playlist.clone())).ok();
                }
            },
            None => {},
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
