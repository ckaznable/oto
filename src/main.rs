use std::{
    cell::Cell,
    collections::VecDeque,
    path::PathBuf,
    rc::Rc,
    sync::mpsc::{Receiver, channel},
};

use alsa::pcm::State;
use anyhow::{Result, anyhow};
use clap::Parser;
use oto::{
    cli,
    decoder::{Decoder, DecoderError, DecoderManager},
    event::PlayerCommand,
    player::Player,
    shared::{RING_BUF_ALLOC, TMP_BUF_ALLOC},
};
use ringbuf::{
    LocalRb,
    storage::Heap,
    traits::{Consumer, Observer, Producer, Split},
};
use tokio::task::{JoinHandle, spawn_blocking};
use walkdir::{DirEntry, WalkDir};

#[tokio::main]
async fn main() -> Result<()> {
    let args = cli::Args::parse();

    let (tx, rx) = channel();

    match args.command {
        cli::Commands::Play { path, device } => {
            let _player_handle: JoinHandle<Result<()>> =
                spawn_blocking(move || player(path, device, rx));
            _player_handle.await?
        }
        cli::Commands::PlayList { command } => {
            todo!()
        }
    }
}

fn player(path: impl Into<PathBuf>, device: String, rx: Receiver<PlayerCommand>) -> Result<()> {
    let rb: LocalRb<Heap<i32>> = LocalRb::new(RING_BUF_ALLOC);
    let (mut prod, mut cons) = rb.split();
    let mut temp_buf = VecDeque::<i32>::with_capacity(TMP_BUF_ALLOC);

    let mut dm = DecoderManager::default();
    dm.open(path.into())?;
    let spec = dm.spec().ok_or(anyhow!("unknown codec"))?;

    let player = Player::new(&device)?;
    player.init(spec)?;

    let spec = Rc::new(Cell::new(spec));

    let mut eof = false;

    loop {
        if let Ok(cmd) = rx.try_recv() {
            match cmd {
                PlayerCommand::Play(media_spec) => {
                    if spec.get() != media_spec {
                        player.drop()?;
                        player.init(media_spec)?;
                        spec.set(media_spec);
                    }
                }
                PlayerCommand::Resume => {
                    player.pause(false)?;
                }
                PlayerCommand::Pause => {
                    player.pause(true)?;
                }
            }
        }

        player.wait(Some(32))?;
        if !matches!(player.state(), State::Running | State::Prepared) {
            player.prepare()?;
        }

        // consume the last data in ring buffer
        if !cons.is_empty() {
            let (right, left) = cons.as_slices();
            let wr = player.write_io(right, spec.get())?;
            let wl = player.write_io(left, spec.get())?;
            cons.skip(wr + wl);
        }

        if !temp_buf.is_empty() {
            let write_to_rb = prod.vacant_len().min(temp_buf.len());
            let data = temp_buf.drain(..write_to_rb);
            prod.push_iter(data);
        }

        if temp_buf.is_empty() {
            temp_buf.shrink_to(TMP_BUF_ALLOC);
        }

        if !prod.is_empty() {
            continue;
        }

        // todo return eof event to controller
        if prod.is_empty() && eof {
            break;
        }

        // todo handle if alsa consumer too slow
        match dm.decode(&mut temp_buf) {
            Ok(_) => {
                let (right, left) = temp_buf.as_slices();
                let wr = player.write_io(right, spec.get())?;
                let wl = player.write_io(left, spec.get())?;
                temp_buf.drain(..(wr + wl));

                // push remaining decoded data to rb
                if !temp_buf.is_empty() {
                    let write_to_rb = prod.vacant_len().min(temp_buf.len());
                    let data = temp_buf.drain(..write_to_rb);
                    prod.push_iter(data);
                }
            }
            Err(DecoderError::EOF) => {
                eof = true;
                continue;
            }
            Err(DecoderError::Ignored) => {}
            Err(_) => {
                continue;
            }
        }

        if !matches!(player.state(), State::Running | State::Paused) {
            player.start()?;
        }
    }

    player.drain()?;
    Ok(())
}

fn all_media_path(p: PathBuf) -> Vec<PathBuf> {
    WalkDir::new(p)
        .into_iter()
        .filter_entry(|e| !is_media_file(e))
        .flatten()
        .map(|e| e.into_path())
        .collect()
}

fn is_media_file(e: &DirEntry) -> bool {
    let p = e.path().extension().and_then(|s| s.to_str());

    matches!(p, Some("flac" | "wav" | "ogg" | "aac" | "mp3" | "dsf"))
}
