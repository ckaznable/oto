use std::ops::Deref;

use anyhow::Result;

use alsa::{
    pcm::{
        HwParams, State
    },
    Direction,
    PCM
};

use crate::media::{MediaSpec, OutputMode};

pub struct Player {
    output: PCM,
}

impl Player {
    pub fn new(device_name: impl AsRef<str>) -> Result<Self> {
        let pcm = PCM::new(device_name.as_ref(), Direction::Playback, false)?;

        Ok(Self {
            output: pcm,
        })
    }

    pub fn set_hw_param(&mut self, spec: MediaSpec) -> Result<()> {
        use OutputMode::*;
        match spec.mode {
            PCM => self.pcm_hw_param(spec.channel, spec.sample_rate),
            DSD => self.dsd_hw_param(spec.channel, spec.sample_rate),
        }
    }

    pub fn pcm_hw_param(&mut self, channel: u32, bit_rate: u32) -> Result<()> {
        let hwp = HwParams::any(&self.output)?;
        hwp.set_channels(channel)?;
        hwp.set_rate(bit_rate, alsa::ValueOr::Nearest)?;
        hwp.set_format(alsa::pcm::Format::S32LE)?;
        hwp.set_access(alsa::pcm::Access::RWInterleaved)?;
        self.output.hw_params(&hwp)?;
        Ok(())
    }

    pub fn dsd_hw_param(&mut self, channel: u32, bit_rate: u32) -> Result<()> {
        let hwp = HwParams::any(&self.output)?;
        hwp.set_channels(channel)?;
        hwp.set_format(alsa::pcm::Format::DSDU32LE)?;
        hwp.set_rate(bit_rate, alsa::ValueOr::Nearest)?;
        hwp.set_access(alsa::pcm::Access::RWInterleaved)?;
        self.output.hw_params(&hwp)?;
        Ok(())
    }

    pub fn set_sw_param(&mut self, spec: MediaSpec) -> Result<()> {
        use OutputMode::*;
        match spec.mode {
            PCM => self.pcm_sw_param(),
            DSD => self.dsd_sw_param(),
        }
    }

    pub fn pcm_sw_param(&mut self) -> Result<()> {
        let swp = self.output.sw_params_current()?;
        let hwp = self.output.hw_params_current()?;
        swp.set_start_threshold(hwp.get_buffer_size().unwrap())?;
        self.output.sw_params(&swp)?;
        Ok(())
    }

    pub fn dsd_sw_param(&mut self) -> Result<()> {
        self.pcm_sw_param()
    }

    pub fn init(&mut self, spec: MediaSpec) -> Result<()> {
        self.set_hw_param(spec)?;
        self.set_sw_param(spec)?;

        let status = self.output.status()?;
        if !matches!(status.get_state(), State::Running | State::Prepared) {
            self.output.prepare()?;
        }

        Ok(())
    }
}

impl Deref for Player {
    type Target = PCM;

    fn deref(&self) -> &Self::Target {
        &self.output
    }
}
