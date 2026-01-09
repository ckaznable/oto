pub const DEFAULT_ALBUM_NAME: &str = "Unknown Album";
pub const DEFAULT_ALBUM_ID: i32 = 1;

#[allow(clippy::upper_case_acronyms)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OutputMode {
    PCM,
    DSD,
}

#[derive(Clone, Debug)]
pub struct Media {
    pub file_path: String,
    pub album: Album,
    pub name: String,
    pub artist: String,
    pub track: u8,
}

#[derive(Clone, Debug)]
pub struct Album {
    pub name: String,
    pub year: u16,
    pub track: u8,
    pub cover: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MediaSpec {
    pub sample_rate: u32,
    pub channels: u32,
    pub mode: OutputMode,
}

