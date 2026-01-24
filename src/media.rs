use std::path::{Path, PathBuf};

use lofty::{config::ParseOptions, file::TaggedFileExt, probe::Probe};
use serde::{Deserialize, Serialize};

use crate::{decoder::DsfReader, util::{cover_from_path, get_cover_with_root_path}};

#[allow(clippy::upper_case_acronyms)]
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum OutputMode {
    #[default]
    PCM,
    DSD,
}

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct Album {
    pub name: Option<String>,
    pub year: Option<u32>,
    pub track: Option<u32>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Default, Serialize, Deserialize)]
pub struct MediaSpec {
    pub sample_rate: u32,
    pub duration: Option<u64>,
    pub channels: u32,
    pub mode: OutputMode,
}

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct TrackMeta {
    pub path: PathBuf,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Album,
    pub duration_secs: u64,
}

impl TrackMeta {
    pub fn empty(path: PathBuf) -> Self {
        Self {
            path,
            ..Default::default()
        }
    }

    pub fn cover(&self) -> Option<Vec<u8>> {
        cover_from_path(&self.path)
    }
}
