use gag::Redirect;
use log::LevelFilter;
use os_pipe::{PipeWriter, pipe};
use simplelog::{TermLogger, WriteLogger};
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
    devices::{get_default_device, list_devices},
    event::{AppCommand, MprisCommand, PickedPlaylist, PlayerCommand},
    mpris,
    player::{BufferPlayer, LastPlayerState, PlayerError},
    volume::VolumeController,
};

#[derive(Default)]
struct PlayerEventLoopConfig {
    path: Option<PathBuf>,
    device: Option<String>,
    play: bool,
}

impl PlayerEventLoopConfig {
    fn new(path: Option<PathBuf>, device: Option<String>) -> Self {
        Self {
            path,
            device,
            play: true,
        }
    }

    fn without_play(path: Option<PathBuf>, device: Option<String>) -> Self {
        let mut config = Self::new(path, device);
        config.play = false;
        config
    }
}

fn main() -> Result<()> {
    let args = cli::Args::parse();

    let (player_tx, player_rx) = channel();
    let (mpris_tx, mpris_rx) = channel();
    let (app_tx, app_rx) = channel();

    let _mpris = mpris::Mpris::handle(player_tx.clone(), mpris_rx)?;

    match args.command {
        #[cfg(feature = "dict-jp")]
        cli::Commands::Init => {
            TermLogger::init(
                LevelFilter::Debug,
                simplelog::Config::default(),
                simplelog::TerminalMode::Mixed,
                simplelog::ColorChoice::Auto,
            )
            .unwrap();

            if let Err(e) = oto::dict::download_and_build_dictionary(true) {
                log::error!("{e}");
            }
            Ok(())
        }
        cli::Commands::Play { path, device } => {
            spawn_mock_app_event_handler(app_rx);
            player_event_loop(
                PlayerEventLoopConfig::new(path, device),
                app_tx,
                mpris_tx,
                player_rx,
            )
        }
        cli::Commands::Tui { path, device } => {
            unsafe {
                std::env::set_var("MALLOC_MMAP_THRESHOLD_", (1024 * 1024 * 2).to_string());
                libc::mallopt(libc::M_MMAP_THRESHOLD, 1024 * 1024 * 2);
            }

            WriteLogger::init(
                LevelFilter::Info,
                simplelog::Config::default(),
                std::fs::File::create("/tmp/oto.log").unwrap(),
            )
            .unwrap();

            let _guard = redirect_stderr_to_log();

            use enclose::enclose;
            std::thread::Builder::new()
                .name("player-event-loop".into())
                .spawn(enclose!((app_tx) move || {
                    if let Err(e) = player_event_loop(PlayerEventLoopConfig::without_play(path, device), app_tx.clone(), mpris_tx, player_rx) {
                        app_tx.send(AppCommand::Unexcepted(e.to_string())).ok();
                        log::error!("{e:?}");
                    }
                }))?;

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

    std::thread::Builder::new()
        .name("redirect-stderr".into())
        .spawn(move || {
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
        })
        .expect("create thread redirect thread failed");

    redirect
}

fn player_event_loop(
    config: PlayerEventLoopConfig,
    tx: Sender<AppCommand>,
    mtx: Sender<MprisCommand>,
    rx: Receiver<PlayerCommand>,
) -> Result<()> {
    let devices = list_devices();

    let PlayerEventLoopConfig {
        path,
        device,
        play: init_play,
    } = config;

    let init_device = device
        .or_else(|| get_default_device(&devices).map(|(p, d)| format!("hw:{p},{d}")))
        .unwrap_or_else(|| "hw:0,0".to_string());

    let mut player = BufferPlayer::new(path, &init_device)?;
    player.init()?;

    let init_spec = player.spec.unwrap();

    let mut vc = VolumeController::new(&init_device);
    let mut volume = vc.get_volume().unwrap_or(0);

    tx.send(AppCommand::VolumeUpdate(volume as u8)).ok();
    mtx.send(MprisCommand::VolumeUpdate(volume as u8)).ok();

    // init tui state
    let mut current_time = 0.;
    if let Some(track) = player.current() {
        tx.send(AppCommand::DevicesList(devices)).ok();
        tx.send(AppCommand::TrackUpdate(track.clone(), init_spec))
            .ok();
        tx.send(AppCommand::PlaylistUpdate(player.playlist.clone()))
            .ok();
        mtx.send(MprisCommand::TrackUpdate(track, init_spec)).ok();
        mtx.send(MprisCommand::PlayBackStateUpdate(current_time, true))
            .ok();

        // hw:0,0 -> 0,0
        let mut device = init_device.clone();
        let mut pcm_index = device.split_off(device.find(":").unwrap() + 1);
        // 0,0 -> 0, 0
        let device_index =
            pcm_index.split_off(pcm_index.find(",").unwrap_or(pcm_index.len() - 1) + 1);
        let device_index = if device_index.is_empty() {
            0i32
        } else {
            device_index.parse::<i32>()?
        };
        // 0, 0 -> 0 0
        pcm_index.truncate(pcm_index.len() - 1);
        let alsa_index = (pcm_index.parse::<i32>()?, device_index);
        tx.send(AppCommand::DeviceUpdate(alsa_index)).ok();
    }

    let mut init = false;

    loop {
        if let Ok(cmd) = rx.try_recv() {
            match cmd {
                PlayerCommand::Play => {
                    player.pause(false)?;
                    mtx.send(MprisCommand::PlayBackStateUpdate(current_time, true))
                        .ok();
                    tx.send(AppCommand::AppModeUpdate(oto::tui::state::AppMode::Playing))
                        .ok();
                }
                PlayerCommand::Pause => {
                    player.pause(true)?;
                    mtx.send(MprisCommand::PlayBackStateUpdate(current_time, false))
                        .ok();
                    tx.send(AppCommand::AppModeUpdate(oto::tui::state::AppMode::Paused))
                        .ok();
                }
                PlayerCommand::PauseCycle => {
                    let pause = !matches!(player.state(), State::Paused);
                    player.pause(pause)?;

                    mtx.send(MprisCommand::PlayBackStateUpdate(current_time, !pause))
                        .ok();
                    tx.send(AppCommand::AppModeUpdate(match pause {
                        true => oto::tui::state::AppMode::Paused,
                        false => oto::tui::state::AppMode::Playing,
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
                        let spec = player.spec.unwrap_or_default();
                        tx.send(AppCommand::TrackUpdate(track.clone(), spec)).ok();
                        tx.send(AppCommand::PlaylistUpdate(player.playlist.clone()))
                            .ok();
                        mtx.send(MprisCommand::TrackUpdate(track, spec)).ok();
                    }
                }
                PlayerCommand::PrevSong => {
                    if let Ok(Some(track)) = player.prev() {
                        let spec = player.spec.unwrap_or_default();
                        tx.send(AppCommand::TrackUpdate(track.clone(), spec)).ok();
                        tx.send(AppCommand::PlaylistUpdate(player.playlist.clone()))
                            .ok();
                        mtx.send(MprisCommand::TrackUpdate(track, spec)).ok();
                    }
                }
                PlayerCommand::PlayTrackWithIndex(index) => {
                    if let Ok(Some(track)) = player.play(index) {
                        let spec = player.spec.unwrap_or_default();
                        tx.send(AppCommand::TrackUpdate(track.clone(), spec)).ok();
                        tx.send(AppCommand::PlaylistUpdate(player.playlist.clone()))
                            .ok();
                        mtx.send(MprisCommand::TrackUpdate(track, spec)).ok();
                        mtx.send(MprisCommand::PlayBackStateUpdate(current_time, true))
                            .ok();
                    }
                }
                PlayerCommand::GetDevices => {
                    tx.send(AppCommand::DevicesList(list_devices())).ok();
                }
                PlayerCommand::SetDevice(d) => {
                    let mut device = format!("hw:{},{}", d.0, d.1);
                    log::info!("try to set {device}");

                    if player.set_device(&device).is_err() {
                        device = init_device.clone();
                    };

                    vc.set_device(&device);
                    tx.send(AppCommand::DeviceUpdate(d)).ok();

                    volume = vc.get_volume().unwrap_or(0);
                    tx.send(AppCommand::VolumeUpdate(volume as u8)).ok();
                    mtx.send(MprisCommand::VolumeUpdate(volume as u8)).ok();

                    player.clear_buffer();
                }
                PlayerCommand::SetPickedPlaylist(picked) => {
                    let is_set_whole_list = matches!(picked, PickedPlaylist::Picked(_));
                    player.playlist.pick(picked);

                    if let Ok(Some(track)) = player.reload()
                        && is_set_whole_list
                    {
                        let spec = player.spec.unwrap_or_default();
                        tx.send(AppCommand::TrackUpdate(track.clone(), spec)).ok();
                        mtx.send(MprisCommand::TrackUpdate(track, spec)).ok();
                        current_time = 0.;
                    }

                    tx.send(AppCommand::PlaylistUpdate(player.playlist.clone()))
                        .ok();
                    mtx.send(MprisCommand::PlayBackStateUpdate(current_time, true))
                        .ok();
                }
            }
        }

        if matches!(player.state(), State::Paused) {
            std::thread::sleep(std::time::Duration::from_millis(100));
            continue;
        }

        player.wait(Some(32))?;
        if !matches!(
            player.state(),
            State::Running | State::Prepared | State::Paused
        ) {
            player.prepare()?;
        }

        current_time = player.calc_duration(match player.delay() {
            Ok(d) => d as u64,
            Err(_) => 0,
        });
        tx.send(AppCommand::TimeUpdate(
            current_time,
            player.spec.and_then(|s| s.duration),
        ))
        .ok();

        if let Err(PlayerError::EOF) = player.consume() {
            break;
        }

        match player.pop_state() {
            Some(LastPlayerState::PlayListChanged) => {
                if let Some(track) = player.current() {
                    tx.send(AppCommand::TrackUpdate(
                        track.clone(),
                        player.spec.unwrap_or_default(),
                    ))
                    .ok();
                    tx.send(AppCommand::PlaylistUpdate(player.playlist.clone()))
                        .ok();
                    mtx.send(MprisCommand::TrackUpdate(
                        track,
                        player.spec.unwrap_or_default(),
                    ))
                    .ok();
                }
            }
            None => {}
        }

        let state = player.state();
        if !matches!(state, State::Running | State::Paused) {
            if let Err(e) = player.start() {
                let e = e.to_string();
                // XRUN
                if e.contains("Broken pipe") {
                    log::warn!("alsa XRUN");
                    player.prepare()?;
                } else {
                    return Err(anyhow::anyhow!(e));
                }
            }

            if !init && !init_play {
                player.pause(true)?;
                mtx.send(MprisCommand::PlayBackStateUpdate(current_time, false))
                    .ok();
                tx.send(AppCommand::AppModeUpdate(oto::tui::state::AppMode::Normal))
                    .ok();
            }
            init = true;
        }
    }

    Ok(())
}
