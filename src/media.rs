use std::path::PathBuf;

use lofty::{file::TaggedFileExt, probe::Probe};

use crate::{decoder::DsfReader, util::get_cover_with_root_path};

#[allow(clippy::upper_case_acronyms)]
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum OutputMode {
    #[default]
    PCM,
    DSD,
}

#[derive(Clone, Default, Debug)]
pub struct Album {
    pub name: Option<String>,
    pub year: Option<u32>,
    pub track: Option<u32>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
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

    pub fn cover(&self) -> Option<Vec<u8>> {
        let tagged_file = Probe::open(&self.path).ok()?.read().ok()?;
        let tag = tagged_file
            .primary_tag()
            .or_else(|| tagged_file.first_tag())?;

        tag.pictures()
            .first()
            .map(|pic| pic.data().to_vec())
            .or_else(|| {
                let mut file = std::fs::File::open(&self.path).ok()?;
                let reader = DsfReader::new(&mut file);
                if let Ok(metadata) = reader.parse() {
                    metadata.tag?.pictures().next().map(|p| p.data.clone())
                } else {
                    Some(get_cover_with_root_path(&self.path)?.1)
                }
            })
    }
}
