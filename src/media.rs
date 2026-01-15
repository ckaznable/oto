use std::path::PathBuf;

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
}
