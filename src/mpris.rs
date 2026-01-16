use std::{
    sync::mpsc::{Receiver, Sender},
    time::Duration,
};

use anyhow::Result;
use souvlaki::{
    MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig,
};

use crate::event::{MprisCommand, PlayerCommand};

const MPRIS_DBUS_NAME: &str = "io.github.ckaznable.oto";
const MPRIS_DISPLAY_NAME: &str = "OTO";

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
                _ => Ok(()),
            }
            .ok();
        })?;

        std::thread::spawn(move || {
            loop {
                if let Ok(event) = rx.recv() {
                    match event {
                        MprisCommand::TrackUpdate(track) => {
                            controls
                                .set_metadata(MediaMetadata {
                                    title: track.title.as_deref(),
                                    artist: track.artist.as_deref(),
                                    album: track.album.name.as_deref(),
                                    duration: Some(Duration::from_secs(track.duration_secs)),
                                    ..Default::default()
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
                    }
                }
            }
        });

        Ok(Self {})
    }
}
