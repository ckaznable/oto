use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    PCM,
    DSD,
}

pub struct Media {
    pub file_path: PathBuf,
}

#[derive(Clone, Copy)]
pub struct MediaSpec {
    pub sample_rate: u32,
    pub channel: u32,
    pub mode: OutputMode,
}

