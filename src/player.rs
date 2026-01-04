use std::{collections::VecDeque, ops::Deref, path::PathBuf};

use anyhow::Result;

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

use crate::{
    decoder::{Decoder, DecoderError, DecoderManager},
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

pub type PlayerResult = Result<(), PlayerError>;

pub struct BufferPlayer {
    rb: LocalRb<Heap<i32>>,
    buf: VecDeque<i32>,
    dm: DecoderManager,
    eof: bool,
    pub spec: Option<MediaSpec>,
}

impl BufferPlayer {
    pub fn new() -> Result<Self> {
        let rb: LocalRb<Heap<i32>> = LocalRb::new(RING_BUF_ALLOC);
        let buf = VecDeque::<i32>::with_capacity(TMP_BUF_ALLOC);

        let dm = DecoderManager::default();

        Ok(Self {
            rb,
            buf,
            dm,
            spec: None,
            eof: false,
        })
    }

    pub fn open(&mut self, p: impl Into<PathBuf>) -> Result<()> {
        self.dm.open(p.into())?;
        self.spec = self.dm.spec();
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
        let Some(spec) = self.spec else {
            return Err(PlayerError::DecoderNotInit);
        };

        // consume the last data in ring buffer
        if !self.rb.is_empty() {
            let (right, left) = self.rb.as_slices();
            let wr = output.write_io(right, spec)?;
            let wl = output.write_io(left, spec)?;
            self.rb.skip(wr + wl);
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
            return Ok(());
        }

        // todo return eof event to controller
        if self.rb.is_empty() && self.eof {
            return Err(PlayerError::EOF);
        }

        // todo handle if alsa consumer too slow
        match self.dm.decode(&mut self.buf) {
            Ok(_) => {
                let (right, left) = self.buf.as_slices();
                let wr = output.write_io(right, spec)?;
                let wl = output.write_io(left, spec)?;
                self.buf.drain(..(wr + wl));

                // push remaining decoded data to rb
                if !self.buf.is_empty() {
                    let write_to_rb = self.rb.vacant_len().min(self.buf.len());
                    let data = self.buf.drain(..write_to_rb);
                    self.rb.push_iter(data);
                }
            }
            Err(DecoderError::EOF) => {
                self.eof = true;
            }
            _ => {}
        }

        Ok(())
    }
}
