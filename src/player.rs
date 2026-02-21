use std::{
    collections::VecDeque,
    ops::{Deref, DerefMut},
    path::PathBuf,
    sync::Arc,
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

use crate::{
    decoder::{Decoder, DecoderError, MixDecoder},
    event::PickedPlaylist,
    media::{MediaSpec, MediaStore, OutputMode, TrackMeta, Tracks},
    shared::{RING_BUF_ALLOC, TMP_BUF_ALLOC},
};

pub struct AudioOutput {
    inner: PCM,
    device_name: String,
}

impl AudioOutput {
    pub fn new(device_name: impl AsRef<str>) -> Result<Self> {
        let inner = PCM::new(device_name.as_ref(), Direction::Playback, false)?;
        let device_name = device_name.as_ref().to_owned();

        Ok(Self { inner, device_name })
    }

    pub fn replace(&mut self, device_name: impl AsRef<str>) -> Result<()> {
        self.inner.pause(true)?;
        self.inner.drop()?;
        self.inner = PCM::new(device_name.as_ref(), Direction::Playback, false)?;
        Ok(())
    }

    pub fn write_io(&self, buf: &[i32], spec: MediaSpec) -> Result<usize> {
        let channel = spec.channels as usize;
        match spec.mode {
            OutputMode::PCM => {
                if let Ok(io) = self.inner.io_i32() {
                    Ok(io.writei(buf)? * channel)
                } else {
                    Ok(0)
                }
            }
            OutputMode::DSD => {
                let io = unsafe { self.inner.io_unchecked::<u32>() };
                Ok(io.writei(cast_slice(buf))? * channel)
            }
        }
    }

    fn set_hw_param(&self, spec: MediaSpec) -> Result<()> {
        self.inner.hw_free()?;
        use OutputMode::*;
        match spec.mode {
            PCM => self.pcm_hw_param(spec.channels, spec.sample_rate),
            DSD => self.dsd_hw_param(spec.channels, spec.sample_rate),
        }
    }

    fn pcm_hw_param(&self, channel: u32, sample_rate: u32) -> Result<()> {
        let hwp = HwParams::any(&self.inner)?;
        hwp.set_access(alsa::pcm::Access::RWInterleaved)?;
        hwp.set_format(alsa::pcm::Format::S32LE)?;
        hwp.set_channels(channel)?;
        hwp.set_rate(sample_rate, alsa::ValueOr::Nearest)?;
        self.inner.hw_params(&hwp)?;
        Ok(())
    }

    fn dsd_hw_param(&self, channel: u32, sample_rate: u32) -> Result<()> {
        let hwp = HwParams::any(&self.inner)?;
        hwp.set_channels(channel)?;
        hwp.set_format(alsa::pcm::Format::DSDU32BE)?;
        hwp.set_rate(sample_rate / 32, alsa::ValueOr::Nearest)?;
        hwp.set_access(alsa::pcm::Access::RWInterleaved)?;
        self.inner.hw_params(&hwp)?;
        Ok(())
    }

    fn set_sw_param(&self, spec: MediaSpec) -> Result<()> {
        use OutputMode::*;
        match spec.mode {
            PCM => self.pcm_sw_param(),
            DSD => self.dsd_sw_param(),
        }
    }

    fn pcm_sw_param(&self) -> Result<()> {
        let swp = self.inner.sw_params_current()?;
        let hwp = self.inner.hw_params_current()?;

        let frames = hwp.get_buffer_size().unwrap();
        log::info!("set alsa buffer {frames} frames");
        swp.set_start_threshold(frames)?;
        self.inner.sw_params(&swp)?;
        Ok(())
    }

    fn dsd_sw_param(&self) -> Result<()> {
        self.pcm_sw_param()
    }

    pub fn init(&self, spec: MediaSpec) -> Result<()> {
        if let Err(e) = self.set_hw_param(spec) {
            log::error!("{e}");
        }

        if let Err(e) = self.set_sw_param(spec) {
            log::error!("{e}");
        }

        if !matches!(
            self.inner.state(),
            State::Running | State::Prepared | State::Paused
        ) {
            self.inner.prepare()?;
        }

        Ok(())
    }
}

impl Deref for AudioOutput {
    type Target = PCM;

    fn deref(&self) -> &Self::Target {
        &self.inner
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

#[derive(Clone, Copy)]
pub enum LastPlayerState {
    PlayListChanged,
}

pub type PlayerResult = Result<usize, PlayerError>;

pub struct BufferPlayer {
    inner: AudioOutput,
    rb: LocalRb<Heap<i32>>,
    buf: VecDeque<i32>,
    decoder: MixDecoder,
    eof: bool,
    written_sample_count: u64,
    last_state: Option<LastPlayerState>,
    pub playlist: PlayList,
    pub spec: Option<MediaSpec>,
}

impl BufferPlayer {
    pub fn new(p: Option<PathBuf>, device: impl AsRef<str>) -> Result<Self> {
        let rb: LocalRb<Heap<i32>> = LocalRb::new(RING_BUF_ALLOC);
        let buf = VecDeque::<i32>::with_capacity(TMP_BUF_ALLOC);

        let decoder = MixDecoder::default();
        let playlist = PlayList::new(p);

        let inner = AudioOutput::new(device)?;

        Ok(Self {
            inner,
            rb,
            buf,
            decoder,
            playlist,
            last_state: None,
            spec: None,
            eof: false,
            written_sample_count: 0,
        })
    }

    pub fn init(&mut self) -> Result<()> {
        let track = self
            .playlist
            .current()
            .ok_or(anyhow!("music file not found"))?;
        self.open_without_init(&track.clone())?;
        self.inner.init(self.spec.unwrap_or_default())?;
        Ok(())
    }

    pub fn clear_buffer(&mut self) {
        self.rb.clear();
        self.buf.clear();
    }

    pub fn pop_state(&mut self) -> Option<LastPlayerState> {
        self.last_state.take()
    }

    pub fn open_without_init(&mut self, track: &TrackMeta) -> Result<()> {
        self.decoder.open(PathBuf::from(&track.path))?;
        self.spec = self.decoder.spec();
        self.written_sample_count = 0;
        Ok(())
    }

    pub fn open(&mut self, track: &TrackMeta) -> Result<()> {
        let old_spec = self.spec;
        self.open_without_init(track)?;
        let new_spec = self.spec;

        if old_spec
            .zip(new_spec)
            .map(|(a, b)| a.sample_rate != b.sample_rate)
            .unwrap_or(false)
        {
            log::info!("sapmle rate diffrent reset hwp and swp");
            self.inner.init(new_spec.unwrap_or_default())?;
            self.inner.prepare()?;
        }

        Ok(())
    }

    pub fn open_immediately(&mut self, track: &TrackMeta) -> Result<()> {
        self.inner.drop()?;
        self.open(track)?;
        self.inner.prepare()?;
        Ok(())
    }

    pub fn play(&mut self, index: usize) -> Result<Option<TrackMeta>> {
        if let Some(track) = self.playlist.play(index) {
            self.open_immediately(&track)?;
            return Ok(Some(track));
        }

        Err(anyhow!("can't play the track with index {}", index))
    }

    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Result<Option<TrackMeta>> {
        if let Some(track) = self.playlist.next() {
            self.open_immediately(&track)?;
            return Ok(Some(track));
        }

        Err(anyhow!("playlist is run dry"))
    }

    pub fn prev(&mut self) -> Result<Option<TrackMeta>> {
        if let Some(track) = self.playlist.prev() {
            self.open_immediately(&track)?;
            return Ok(Some(track));
        }

        Ok(None)
    }

    pub fn reload(&mut self) -> Result<Option<TrackMeta>> {
        if let Some(track) = self.current()
            && let Some(ref path) = self.decoder.file_path
            && path != &track.path
        {
            self.open_immediately(&track)?;
            return Ok(Some(track));
        }

        Ok(None)
    }

    pub fn current(&mut self) -> Option<TrackMeta> {
        self.playlist.current().cloned()
    }

    pub fn consume(&mut self) -> PlayerResult {
        let mut written = 0usize;

        let Some(spec) = self.spec else {
            return Err(PlayerError::DecoderNotInit);
        };

        // consume the last data in ring buffer
        if !self.rb.is_empty() {
            let (right, left) = self.rb.as_slices();
            let wr = self.inner.write_io(right, spec)?;
            let wl = self.inner.write_io(left, spec)?;
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
            self.written_sample_count += written as u64;
            return Ok(written);
        }

        if self.rb.is_empty() && self.eof {
            return Err(PlayerError::EOF);
        }

        match self.decoder.decode(&mut self.buf) {
            Ok(_) => {
                let (right, left) = self.buf.as_slices();
                let wr = self.inner.write_io(right, spec)?;
                let wl = self.inner.write_io(left, spec)?;
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

                match self.next() {
                    Err(e) => {
                        log::error!("{e:?}");
                        self.eof = true;
                    }
                    Ok(_) => {
                        self.last_state.replace(LastPlayerState::PlayListChanged);
                    }
                }
            }
            _ => {}
        }

        self.written_sample_count += written as u64;
        Ok(written)
    }

    pub fn calc_duration(&self, delay: u64) -> f64 {
        let Some((written_frames, sample_rate)) = self.get_written_frames() else {
            return 0.;
        };

        let actual_played_frames = written_frames.saturating_sub(delay);
        actual_played_frames as f64 / sample_rate as f64
    }

    pub fn get_written_frames(&self) -> Option<(u64, u32)> {
        match self.spec? {
            MediaSpec {
                sample_rate,
                mode: OutputMode::PCM,
                channels,
                ..
            } => Some((self.written_sample_count / channels as u64, sample_rate)),
            MediaSpec {
                sample_rate,
                mode: OutputMode::DSD,
                channels,
                ..
            } => Some((
                self.written_sample_count * 32 / channels as u64,
                sample_rate,
            )),
        }
    }

    pub fn get_samples_from_frames(&self, frames: u64) -> Option<u64> {
        match self.spec? {
            MediaSpec {
                mode: OutputMode::PCM,
                channels,
                ..
            } => Some(frames * channels as u64),
            MediaSpec {
                mode: OutputMode::DSD,
                channels,
                ..
            } => Some(frames * channels as u64 / 32),
        }
    }

    pub fn pause(&mut self, pause: bool) -> Result<()> {
        if matches!(self.inner.state(), State::Suspended) {
            self.inner.resume()?;
        }

        self.inner.pause(pause)?;
        Ok(())
    }

    pub fn set_device(&mut self, device: impl AsRef<str>) -> Result<()> {
        if self.inner.replace(device.as_ref()).is_err() {
            let device = self.inner.device_name.clone();
            self.inner.replace(&device)?;
            return Err(anyhow!("set device failed"));
        }

        self.inner.init(self.spec.unwrap_or_default())?;
        Ok(())
    }
}

impl Deref for BufferPlayer {
    type Target = AudioOutput;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for BufferPlayer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

#[derive(Clone, Default)]
pub struct PlayList {
    pub list: Arc<Tracks>,
    pub picked: Arc<Option<Vec<usize>>>,
    pub index: usize,
}

impl PlayList {
    pub fn new(p: Option<PathBuf>) -> Self {
        Self {
            list: Arc::new(MediaStore::get_tracks_with_cache(p.as_deref())),
            picked: Arc::new(None),
            index: 0,
        }
    }

    pub fn playing_index(&self, index: usize) -> usize {
        if let Some(p) = self.picked.as_deref() {
            p.get(index).cloned().unwrap_or_default()
        } else {
            index
        }
    }

    pub fn list_len(&self) -> usize {
        if let Some(p) = self.picked.as_deref() {
            p.len()
        } else {
            self.list.len()
        }
    }

    pub fn pick(&mut self, picked: PickedPlaylist) {
        match picked {
            PickedPlaylist::Picked(items) => {
                self.index = 0;
                self.picked = Arc::new(if items.is_empty() { None } else { Some(items) });
            }
            PickedPlaylist::InsertNext(items) => {
                self.init_picked();
                let mut new_picked = self.picked.as_deref().unwrap().to_vec();
                let mut tail = new_picked.split_off(self.index + 1);
                new_picked.extend(items);
                new_picked.append(&mut tail);
                self.picked = Arc::new(Some(new_picked));
            }
            PickedPlaylist::Append(items) => {
                self.init_picked();
                let mut new_picked = self.picked.as_deref().unwrap().to_vec();
                new_picked.extend(items);
                self.picked = Arc::new(Some(new_picked));
            }
        }
    }

    fn init_picked(&mut self) {
        if self.picked.is_none() {
            self.picked = Arc::new(Some(self.gen_picked()));
        }
    }

    pub fn gen_picked(&self) -> Vec<usize> {
        (0..self.list.len()).collect()
    }

    pub fn play(&mut self, index: usize) -> Option<TrackMeta> {
        let track = self.list.get(self.playing_index(index)).cloned()?;
        self.index = index;
        Some(track)
    }

    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Option<TrackMeta> {
        let next = self.index + 1;
        if next >= self.list_len() {
            return None;
        }

        self.index = next;
        self.list.get(self.playing_index(self.index)).cloned()
    }

    pub fn prev(&mut self) -> Option<TrackMeta> {
        self.index = self.index.saturating_sub(1);
        self.list.get(self.playing_index(self.index)).cloned()
    }

    #[inline]
    pub fn current(&self) -> Option<&TrackMeta> {
        self.list.get(self.playing_index(self.index))
    }

    #[inline]
    pub fn is_end(&self) -> bool {
        if let Some(p) = self.picked.as_deref() {
            p.len() == self.index.saturating_sub(1)
        } else {
            self.list.len() == self.index.saturating_sub(1)
        }
    }
}
