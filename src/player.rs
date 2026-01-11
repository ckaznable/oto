use std::{
    collections::VecDeque,
    ops::Deref,
    path::{Path, PathBuf},
};

use anyhow::{Result, anyhow};

use alsa::{
    Direction, PCM,
    pcm::{HwParams, State},
};
use bytemuck::cast_slice;
use ringbuf::{
    LocalRb,
    storage::Heap,
    traits::{Consumer, Observer, Producer},
};
use walkdir::{DirEntry, WalkDir};

use crate::{
    decoder::{Decoder, DecoderError, MixDecoder},
    media::{MediaSpec, OutputMode},
    shared::{RING_BUF_ALLOC, TMP_BUF_ALLOC},
};

pub struct AudioOutput {
    output: PCM,
}

impl AudioOutput {
    pub fn new(device_name: impl AsRef<str>) -> Result<Self> {
        let output = PCM::new(device_name.as_ref(), Direction::Playback, false)?;

        Ok(Self { output })
    }

    pub fn write_io(&self, buf: &[i32], spec: MediaSpec) -> Result<usize> {
        let channel = spec.channels as usize;
        match spec.mode {
            OutputMode::PCM => {
                if let Ok(io) = self.output.io_i32() {
                    Ok(io.writei(buf)? * channel)
                } else {
                    Ok(0)
                }
            }
            OutputMode::DSD => {
                let io = unsafe { self.output.io_unchecked::<u32>() };
                Ok(io.writei(cast_slice(buf))? * channel)
            }
        }
    }

    pub fn set_hw_param(&self, spec: MediaSpec) -> Result<()> {
        use OutputMode::*;
        match spec.mode {
            PCM => self.pcm_hw_param(spec.channels, spec.sample_rate),
            DSD => self.dsd_hw_param(spec.channels, spec.sample_rate),
        }
    }

    pub fn pcm_hw_param(&self, channel: u32, sample_rate: u32) -> Result<()> {
        let hwp = HwParams::any(&self.output)?;
        hwp.set_channels(channel)?;
        hwp.set_rate(sample_rate, alsa::ValueOr::Nearest)?;
        hwp.set_format(alsa::pcm::Format::S32LE)?;
        hwp.set_access(alsa::pcm::Access::RWInterleaved)?;
        self.output.hw_params(&hwp)?;
        Ok(())
    }

    pub fn dsd_hw_param(&self, channel: u32, sample_rate: u32) -> Result<()> {
        let hwp = HwParams::any(&self.output)?;
        hwp.set_channels(channel)?;
        hwp.set_format(alsa::pcm::Format::DSDU32BE)?;
        hwp.set_rate(sample_rate / 32, alsa::ValueOr::Nearest)?;
        hwp.set_access(alsa::pcm::Access::RWInterleaved)?;
        self.output.hw_params(&hwp)?;
        Ok(())
    }

    pub fn set_sw_param(&self, spec: MediaSpec) -> Result<()> {
        use OutputMode::*;
        match spec.mode {
            PCM => self.pcm_sw_param(),
            DSD => self.dsd_sw_param(),
        }
    }

    pub fn pcm_sw_param(&self) -> Result<()> {
        let swp = self.output.sw_params_current()?;
        let hwp = self.output.hw_params_current()?;
        swp.set_start_threshold(hwp.get_buffer_size().unwrap())?;
        self.output.sw_params(&swp)?;
        Ok(())
    }

    pub fn dsd_sw_param(&self) -> Result<()> {
        self.pcm_sw_param()
    }

    pub fn init(&self, spec: MediaSpec) -> Result<()> {
        self.set_hw_param(spec)?;
        self.set_sw_param(spec)?;

        let status = self.output.status()?;
        if !matches!(status.get_state(), State::Running | State::Prepared) {
            self.output.prepare()?;
        }

        Ok(())
    }
}

impl Deref for AudioOutput {
    type Target = PCM;

    fn deref(&self) -> &Self::Target {
        &self.output
    }
}

#[derive(thiserror::Error, Debug)]
pub enum PlayerError {
    #[error("eof")]
    EOF,
    #[error("decoder is init yet, call open to init")]
    DecoderNotInit,
    #[error("{0}")]
    Unexcepted(String),
}

impl From<anyhow::Error> for PlayerError {
    fn from(value: anyhow::Error) -> Self {
        Self::Unexcepted(value.to_string())
    }
}

pub type PlayerResult = Result<usize, PlayerError>;

pub struct BufferPlayer {
    rb: LocalRb<Heap<i32>>,
    buf: VecDeque<i32>,
    decoder: MixDecoder,
    eof: bool,
    playlist: PlayList,
    written_sample_count: u64,
    pub spec: Option<MediaSpec>,
}

impl BufferPlayer {
    pub fn new(p: impl Into<PathBuf>) -> Result<Self> {
        let rb: LocalRb<Heap<i32>> = LocalRb::new(RING_BUF_ALLOC);
        let buf = VecDeque::<i32>::with_capacity(TMP_BUF_ALLOC);

        let decoder = MixDecoder::default();
        let playlist = PlayList::new(p);

        Ok(Self {
            rb,
            buf,
            decoder,
            playlist,
            spec: None,
            eof: false,
            written_sample_count: 0,
        })
    }

    pub fn init(&mut self) -> Result<()> {
        let p = self
            .playlist
            .current()
            .ok_or(anyhow!("music file not found"))?;
        self.open(p)
    }

    pub fn open(&mut self, p: impl Into<PathBuf>) -> Result<()> {
        self.decoder.open(p.into())?;
        self.spec = self.decoder.spec();
        self.written_sample_count = 0;
        Ok(())
    }

    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Result<()> {
        if let Some(p) = self.playlist.next() {
            self.open(p)?;
            return Ok(());
        }

        Err(anyhow!("playlist is run dry"))
    }

    pub fn prev(&mut self) -> Result<()> {
        if let Some(p) = self.playlist.prev() {
            self.open(p)?;
        }

        Ok(())
    }

    pub fn set_spec(&mut self, media_spec: MediaSpec, output: &mut AudioOutput) -> Result<()> {
        if let Some(spec) = self.spec
            && spec != media_spec
        {
            output.drop()?;
            output.init(media_spec)?;
            self.spec = Some(media_spec);
        }

        Ok(())
    }

    pub fn consume(&mut self, output: &mut AudioOutput) -> PlayerResult {
        let mut written = 0usize;

        let Some(spec) = self.spec else {
            return Err(PlayerError::DecoderNotInit);
        };

        // consume the last data in ring buffer
        if !self.rb.is_empty() {
            let (right, left) = self.rb.as_slices();
            let wr = output.write_io(right, spec)?;
            let wl = output.write_io(left, spec)?;
            self.rb.skip(wr + wl);
            written += wr + wl;
        }

        if !self.buf.is_empty() {
            let write_to_rb = self.rb.vacant_len().min(self.buf.len());
            let data = self.buf.drain(..write_to_rb);
            self.rb.push_iter(data);
        }

        if self.buf.is_empty() {
            self.buf.shrink_to(TMP_BUF_ALLOC);
        }

        if !self.rb.is_empty() {
            return Ok(written);
        }

        if self.rb.is_empty() && self.eof {
            return Err(PlayerError::EOF);
        }

        match self.decoder.decode(&mut self.buf) {
            Ok(_) => {
                let (right, left) = self.buf.as_slices();
                let wr = output.write_io(right, spec)?;
                let wl = output.write_io(left, spec)?;
                self.buf.drain(..(wr + wl));
                written += wr + wl;

                // push remaining decoded data to rb
                if !self.buf.is_empty() {
                    let write_to_rb = self.rb.vacant_len().min(self.buf.len());
                    let data = self.buf.drain(..write_to_rb);
                    self.rb.push_iter(data);
                }
            }
            Err(DecoderError::EOF) => {
                if self.playlist.is_end() {
                    self.eof = true;
                }

                if let Err(e) = self.next() {
                    log::error!("{e:?}");
                    self.eof = true;
                }
            }
            _ => {}
        }

        self.written_sample_count += written as u64;
        Ok(written)
    }

    pub fn calc_duration(&self) -> f64 {
        match self.spec {
            None => 0.,
            Some(MediaSpec { sample_rate, mode: OutputMode::PCM, .. }) => {
                self.written_sample_count as f64 / sample_rate as f64
            }
            Some(MediaSpec { sample_rate, mode: OutputMode::DSD, .. }) => {
                self.written_sample_count as f64 * 32. / sample_rate as f64
            }
        }
    }
}

#[derive(Clone)]
pub struct PlayList {
    list: Vec<PathBuf>,
    index: usize,
}

impl PlayList {
    pub fn new(p: impl Into<PathBuf>) -> Self {
        let p = p.into();
        let mut list = Self {
            list: vec![],
            index: 0,
        };

        if !p.exists() {
            return list;
        }

        if p.is_file() {
            list.list.push(p);
        } else {
            list.list = all_media_path(&p);
        }

        list
    }

    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Option<PathBuf> {
        let next = self.index + 1;
        if next >= self.list.len() {
            return None;
        }

        self.index = next;
        self.list.get(next).cloned()
    }

    pub fn prev(&mut self) -> Option<PathBuf> {
        self.index = self.index.saturating_sub(1);
        self.list.get(self.index).cloned()
    }

    #[inline]
    pub fn current(&self) -> Option<PathBuf> {
        self.list.get(self.index).cloned()
    }

    #[inline]
    pub fn is_end(&self) -> bool {
        self.list.len() == self.index.saturating_sub(1)
    }
}

fn all_media_path(p: &Path) -> Vec<PathBuf> {
    WalkDir::new(p)
        .into_iter()
        .flatten()
        .filter(is_media_file)
        .map(|e| e.into_path())
        .collect()
}

fn is_media_file(e: &DirEntry) -> bool {
    let p = e.path().extension().and_then(|s| s.to_str());
    matches!(p, Some("flac" | "wav" | "ogg" | "aac" | "mp3" | "dsf"))
}
