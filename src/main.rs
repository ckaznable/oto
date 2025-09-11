use std::{collections::VecDeque, path::PathBuf};

use alsa::pcm::State;
use anyhow::{anyhow, Result};
use ringbuf::{storage::Heap, traits::{Consumer, Observer, Producer, Split}, LocalRb};
use tokio::task::{spawn_blocking, JoinHandle};

use crate::{
    decoder::{Decoder, DecoderError, DecoderManager},
    player::Player
};

mod decoder;
mod event;
mod media;
mod player;

const I32_BYTE: usize = i32::BITS as usize / 8;

// 256kb i32
const TMP_BUF_ALLOC: usize = (1024 * 256) / I32_BYTE;

// 4mb i32
const RING_BUF_ALLOC: usize = (1024 * 1024 * 4) / I32_BYTE;

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let path = args.get(1).expect("file path not provided").clone();
    let device = args.get(2).expect("alsa device not provided").clone();

    let _player_handle: JoinHandle<Result<()>> = spawn_blocking(move || player(path, device));
    _player_handle.await?
}

fn player(path: impl Into<PathBuf>, device: String) -> Result<()> {
    let rb: LocalRb<Heap<i32>> = LocalRb::new(RING_BUF_ALLOC);
    let (mut prod, mut cons) = rb.split();
    let mut temp_buf = VecDeque::<i32>::with_capacity(TMP_BUF_ALLOC);

    let mut dm = DecoderManager::default();
    dm.open(path.into())?;
    let spec = dm.spec().ok_or(anyhow!("unknown codec"))?;
    let channel = spec.channel as usize;

    let mut player = Player::new(&device)?;
    player.init(spec)?;
    let io = player.io_i32()?;

    let mut eof = false;

    loop {
        player.wait(None)?;
        if !matches!(player.state(), State::Running | State::Prepared) {
            player.prepare()?;
        }

        // consume the last data in ring buffer
        if !cons.is_empty() {
            let (right, left) = cons.as_slices();
            let wr = io.writei(right)? * channel;
            let wl = io.writei(left)? * channel;
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

        if prod.is_empty() && eof {
            break;
        }

        match dm.decode(&mut temp_buf) {
            Ok(_) => {
                let (right, left) = temp_buf.as_slices();
                let wr = io.writei(right)? * channel;
                let wl = io.writei(left)? * channel;
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

        if !matches!(player.state(), State::Running) {
            player.start()?;
        }
    }

    player.drain()?;
    Ok(())
}
