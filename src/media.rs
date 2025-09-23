use std::path::PathBuf;

#[allow(clippy::upper_case_acronyms)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OutputMode {
    PCM,
    DSD,
}

pub struct Media {
    pub file_path: PathBuf,
}

#[derive(Clone, Copy, Debug)]
pub struct MediaSpec {
    pub sample_rate: u32,
    pub channel: u32,
    pub mode: OutputMode,
}

