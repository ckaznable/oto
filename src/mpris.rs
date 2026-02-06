use std::{
    io::Write,
    sync::mpsc::{Receiver, Sender},
    time::Duration,
};

use anyhow::Result;
use souvlaki::{
    MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig,
};

use crate::{
    event::{MprisCommand, PlayerCommand},
    shared::PROJ_DIRS,
};

const MPRIS_DBUS_NAME: &str = "io.github.ckaznable.oto";
const MPRIS_DISPLAY_NAME: &str = "OTO";

const COVER_CACHE_FILE_NAME: &str = "cover";

pub struct Mpris {}

impl Mpris {
    pub fn handle(tx: Sender<PlayerCommand>, rx: Receiver<MprisCommand>) -> Result<Self> {
        let mut controls = MediaControls::new(PlatformConfig {
            dbus_name: MPRIS_DBUS_NAME,
            display_name: MPRIS_DISPLAY_NAME,
            hwnd: None,
        })?;

        controls.attach(move |event: MediaControlEvent| {
            match event {
                MediaControlEvent::Play => tx.send(PlayerCommand::Play),
                MediaControlEvent::Pause => tx.send(PlayerCommand::Pause),
                MediaControlEvent::Toggle => tx.send(PlayerCommand::PauseCycle),
                MediaControlEvent::Next => tx.send(PlayerCommand::NextSong),
                MediaControlEvent::Previous => tx.send(PlayerCommand::PrevSong),
                MediaControlEvent::SetVolume(vol) => tx.send(PlayerCommand::SetVolumn(
                    (vol * 100.).clamp(0., 100.).floor() as u8,
                )),
                _ => Ok(()),
            }
            .ok();
        })?;

        std::fs::create_dir_all(PROJ_DIRS.cache_dir())?;

        std::thread::Builder::new()
            .name("mpris".into())
            .spawn(move || {
                loop {
                    if let Ok(event) = rx.recv() {
                        match event {
                            MprisCommand::TrackUpdate(track, _) => {
                                let cover_url = track.cover().and_then(|data| {
                                    let cover_file_path =
                                        PROJ_DIRS.cache_dir().join(COVER_CACHE_FILE_NAME);
                                    let mut f = std::fs::File::create(&cover_file_path).ok()?;
                                    f.write_all(&data).ok();
                                    Some(cover_file_path)
                                });

                                controls
                                    .set_metadata(MediaMetadata {
                                        title: track.title.as_deref(),
                                        artist: track.artist.as_deref(),
                                        album: track.album.name.as_deref(),
                                        duration: Some(Duration::from_secs(track.duration_secs)),
                                        cover_url: cover_url
                                            .as_ref()
                                            .and_then(|p| p.as_os_str().to_str()),
                                    })
                                    .ok();
                            }
                            MprisCommand::PlayBackStateUpdate(c, p) => {
                                let progress = Some(MediaPosition(Duration::from_secs_f64(c)));
                                controls
                                    .set_playback(if p {
                                        MediaPlayback::Playing { progress }
                                    } else {
                                        MediaPlayback::Paused { progress }
                                    })
                                    .ok();
                            }
                            MprisCommand::VolumeUpdate(v) => {
                                controls.set_volume(v as f64 / 100.).ok();
                            }
                        }
                    }
                }
            })?;

        Ok(Self {})
    }
}
