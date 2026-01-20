use std::{
    collections::VecDeque,
    ops::Deref,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{Result, anyhow};

use alsa::{
    Direction, PCM,
    pcm::{HwParams, State},
};
use bytemuck::cast_slice;
use id3::TagLike;
use lofty::{
    config::ParseOptions,
    file::{AudioFile, TaggedFileExt},
    probe::Probe,
    tag::Accessor,
};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use ringbuf::{
    LocalRb,
    storage::Heap,
    traits::{Consumer, Observer, Producer},
};
use walkdir::WalkDir;

use crate::{
    decoder::{Decoder, DecoderError, DsfReader, MixDecoder},
    media::{Album, MediaSpec, OutputMode, TrackMeta},
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

#[derive(Clone, Copy)]
pub enum LastPlayerState {
    PlayListChanged,
}

pub type PlayerResult = Result<usize, PlayerError>;

pub struct BufferPlayer {
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
        self.open(track.path.clone())
    }

    pub fn clear_buffer(&mut self) {
        self.rb.clear();
        self.buf.clear();
    }

    pub fn pop_state(&mut self) -> Option<LastPlayerState> {
        self.last_state.take()
    }

    pub fn open(&mut self, p: impl Into<PathBuf>) -> Result<()> {
        self.decoder.open(p.into())?;
        self.spec = self.decoder.spec();
        self.written_sample_count = 0;
        Ok(())
    }

    pub fn play(&mut self, index: usize) -> Result<Option<TrackMeta>> {
        if let Some(track) = self.playlist.play(index) {
            self.open(&track.path)?;
            return Ok(Some(track));
        }

        Err(anyhow!("can't play the track with index {}", index))
    }

    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Result<Option<TrackMeta>> {
        if let Some(track) = self.playlist.next() {
            self.open(&track.path)?;
            return Ok(Some(track));
        }

        Err(anyhow!("playlist is run dry"))
    }

    pub fn prev(&mut self) -> Result<Option<TrackMeta>> {
        if let Some(track) = self.playlist.prev() {
            self.open(&track.path)?;
            return Ok(Some(track));
        }

        Ok(None)
    }

    pub fn current(&mut self) -> Option<TrackMeta> {
        self.playlist.current().cloned()
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
            self.written_sample_count += written as u64;
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
        let (written_frames, sample_rate) = match self.spec {
            None => return 0.,
            Some(MediaSpec {
                sample_rate,
                mode: OutputMode::PCM,
                channels,
                ..
            }) => (self.written_sample_count / channels as u64, sample_rate),
            Some(MediaSpec {
                sample_rate,
                mode: OutputMode::DSD,
                channels,
                ..
            }) => (
                self.written_sample_count * 32 / channels as u64,
                sample_rate,
            ),
        };

        let actual_played_frames = written_frames.saturating_sub(delay);
        actual_played_frames as f64 / sample_rate as f64
    }
}

#[derive(Clone, Default)]
pub struct PlayList {
    pub list: Arc<Vec<TrackMeta>>,
    pub index: usize,
}

impl PlayList {
    pub fn new(p: impl Into<PathBuf>) -> Self {
        let p = p.into();
        let mut list = vec![];

        if !p.exists() {
            return Self::default();
        }

        if p.is_file()
            && let Some(track) = parse_one_file(&p)
        {
            list.push(track);
        } else {
            list = scan_music_library(&p);
        }

        Self {
            list: Arc::new(list),
            index: 0,
        }
    }

    pub fn play(&mut self, index: usize) -> Option<TrackMeta> {
        let track = self.list.get(index).cloned()?;
        self.index = index;
        Some(track)
    }

    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Option<TrackMeta> {
        let next = self.index + 1;
        if next >= self.list.len() {
            return None;
        }

        self.index = next;
        self.list.get(next).cloned()
    }

    pub fn prev(&mut self) -> Option<TrackMeta> {
        self.index = self.index.saturating_sub(1);
        self.list.get(self.index).cloned()
    }

    #[inline]
    pub fn current(&self) -> Option<&TrackMeta> {
        self.list.get(self.index)
    }

    #[inline]
    pub fn is_end(&self) -> bool {
        self.list.len() == self.index.saturating_sub(1)
    }
}

pub fn scan_music_library(root_path: &Path) -> Vec<TrackMeta> {
    let files: Vec<_> = WalkDir::new(root_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path().extension().is_some_and(|ext| {
                ext == "flac"
                    || ext == "dsf"
                    || ext == "acc"
                    || ext == "mp3"
                    || ext == "ogg"
                    || ext == "wav"
            })
        })
        .collect();

    log::info!("found media files: {}", files.len());

    let tracks: Vec<TrackMeta> = files
        .par_iter()
        .map(|entry| {
            let path = entry.path();
            parse_one_file(path)
        })
        .flatten()
        .collect();

    log::info!("found tracks: {}", tracks.len());

    tracks
}

pub fn parse_dsf_file(path: &Path) -> Option<TrackMeta> {
    let mut file = std::fs::File::open(path).ok()?;
    let reader = DsfReader::new(&mut file);
    let metadata = reader.parse().ok()?;
    let duration_secs =
        (metadata.sample_count / metadata.channel_num as u64) / metadata.sample_freq as u64;
    let tag = metadata.tag?;

    let track = TrackMeta {
        path: path.to_owned(),
        title: tag.title().map(|a| a.to_string()),
        artist: tag.artist().map(|a| a.to_string()),
        album: Album {
            name: tag.album().map(|a| a.to_string()),
            year: tag.year().map(|a| a as u32),
            track: tag.track(),
        },
        duration_secs,
    };

    Some(track)
}

pub fn parse_one_file(path: &Path) -> Option<TrackMeta> {
    if let Some(ext) = path.extension()
        && ext == "dsf"
    {
        return parse_dsf_file(path);
    }

    let options = ParseOptions::new().read_cover_art(false);
    let tagged_file = Probe::open(path).ok()?.options(options).read().ok()?;

    let tag = tagged_file
        .primary_tag()
        .or_else(|| tagged_file.first_tag())?;

    let properties = tagged_file.properties();

    Some(TrackMeta {
        path: path.to_owned(),
        title: tag.title().map(|t| t.to_string()),
        artist: tag.artist().map(|a| a.to_string()),
        album: Album {
            name: tag.album().map(|a| a.to_string()),
            year: tag.year(),
            track: tag.track(),
        },
        duration_secs: properties.duration().as_secs(),
    })
}
