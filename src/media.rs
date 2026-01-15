use std::path::{Path, PathBuf};

use lofty::{file::TaggedFileExt, probe::Probe};

use crate::util::get_cover_with_root_path;

#[allow(clippy::upper_case_acronyms)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OutputMode {
    PCM,
    DSD,
}

#[derive(Clone, Default, Debug)]
pub struct Album {
    pub name: Option<String>,
    pub year: Option<u32>,
    pub track: Option<u32>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MediaSpec {
    pub sample_rate: u32,
    pub duration: Option<u64>,
    pub channels: u32,
    pub mode: OutputMode,
}

#[derive(Clone, Default, Debug)]
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

    // todo dsd handle
    pub fn cover(&self, path: &Path) -> Option<Vec<u8>> {
        let tagged_file = Probe::open(path).ok()?.read().ok()?;
        let tag = tagged_file
            .primary_tag()
            .or_else(|| tagged_file.first_tag())?;

        tag.pictures()
            .first()
            .map(|pic| pic.data().to_vec())
            .or_else(|| Some(get_cover_with_root_path(path)?.1))
    }
}
