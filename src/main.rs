use std::{cell::RefCell, collections::VecDeque, path::PathBuf, rc::Rc, sync::mpsc::{channel, Receiver}};

use alsa::pcm::State;
use anyhow::{anyhow, Result};
use clap::Parser;
use ringbuf::{
    storage::Heap,
    traits::{Consumer, Observer, Producer, Split},
    LocalRb
};
use tokio::task::{spawn_blocking, JoinHandle};
use walkdir::{DirEntry, WalkDir};

use crate::{
    decoder::{Decoder, DecoderError, DecoderManager}, event::PlayerCommand, player::Player
};

mod cli;
mod decoder;
mod event;
mod media;
mod player;
mod shared;
mod store;

const I32_BYTE: usize = i32::BITS as usize / 8;

// 256kb i32
const TMP_BUF_ALLOC: usize = (1024 * 256) / I32_BYTE;

// 4mb i32
const RING_BUF_ALLOC: usize = (1024 * 1024 * 4) / I32_BYTE;

#[tokio::main]
async fn main() -> Result<()> {
    let args = cli::Args::parse();

    let (tx, rx) = channel();

    match args.command {
        cli::Commands::Play { path, device } => {
            let _player_handle: JoinHandle<Result<()>> = spawn_blocking(move || player(path, device, rx));
            _player_handle.await?
        },
        cli::Commands::PlayList { command } => {
            todo!()
        },
    }
}

fn player(path: impl Into<PathBuf>, device: String, rx: Receiver<PlayerCommand>) -> Result<()> {
    let rb: LocalRb<Heap<i32>> = LocalRb::new(RING_BUF_ALLOC);
    let (mut prod, mut cons) = rb.split();
    let mut temp_buf = VecDeque::<i32>::with_capacity(TMP_BUF_ALLOC);

    let mut dm = DecoderManager::default();
    dm.open(path.into())?;
    let spec = dm.spec().ok_or(anyhow!("unknown codec"))?;
    let channel = spec.channel as usize;

    let player = Player::new(&device)?;
    player.init(spec)?;
    let io = Rc::new(RefCell::new(Some(player.io_i32())));
    let io_dsd = Rc::new(RefCell::new(Some(player.io_u32())));

    let spec = Rc::new(RefCell::new(spec));

    let spec_in_fn = spec.clone();
    let io_in_fn = io.clone();
    let io_dsd_in_fn = io_dsd.clone();

    #[allow(clippy::type_complexity)]
    let write_io: Box<dyn Fn(&[i32]) -> anyhow::Result<usize>> = Box::new(move |buf: &[i32]| {
        match spec_in_fn.borrow().mode {
            media::OutputMode::PCM => {
                if let Some(Ok(io)) = &*io_in_fn.borrow() {
                    Ok(io.writei(buf)? * channel)
                } else {
                    Ok(0)
                }
            },
            media::OutputMode::DSD => {
                let buf = unsafe {
                    std::slice::from_raw_parts(
                        buf.as_ptr() as *const u32,
                        buf.len()
                    )
                };

                if let Some(Ok(io)) = &*io_dsd_in_fn.borrow() {
                    Ok(io.writei(buf)? * channel)
                } else {
                    Ok(0)
                }
            },
        }
    });

    let mut eof = false;

    loop {
        if let Ok(cmd) = rx.try_recv() {
            match cmd {
                PlayerCommand::Play(media_spec) => {
                    player.drop()?;
                    player.init(media_spec)?;
                    let mut spec = spec.borrow_mut();
                    *spec = media_spec;

                    drop(io.take());
                    *io.borrow_mut() = Some(player.io_i32());
                    drop(io_dsd.take());
                    *io_dsd.borrow_mut() = Some(player.io_u32());
                },
                PlayerCommand::Resume => {
                    player.pause(false)?;
                },
                PlayerCommand::Pause => {
                    player.pause(true)?;
                },
            }
        }

        player.wait(Some(32))?;
        if !matches!(player.state(), State::Running | State::Prepared) {
            player.prepare()?;
        }

        // consume the last data in ring buffer
        if !cons.is_empty() {
            let (right, left) = cons.as_slices();
            let wr = write_io(right)?;
            let wl = write_io(left)?;
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

        match dm.decode(&mut temp_buf) {
            Ok(_) => {
                let (right, left) = temp_buf.as_slices();
                let wr = write_io(right)?;
                let wl = write_io(left)?;
                temp_buf.drain(..(wr + wl));

                if !temp_buf.is_empty() {
                    let write_to_rb = prod.vacant_len().min(temp_buf.len());
                    let data = temp_buf.drain(..write_to_rb);
                    prod.push_iter(data);
                }
            },
            Err(DecoderError::EOF) => {
                eof = true;
                continue;
            },
            Err(DecoderError::Ignored) => { },
            Err(_) => {
                continue;
            },
        }

        if !matches!(player.state(), State::Running|State::Paused) {
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
    let p = e.path()
        .extension()
        .and_then(|s| s.to_str());

    matches!(p, Some("flac"|"wav"|"ogg"|"aac"|"mp3"))
}
