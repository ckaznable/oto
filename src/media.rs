pub const DEFAULT_ALBUM_NAME: &str = "Unknown Album";
pub const DEFAULT_ALBUM_ID: i32 = 1;

#[allow(clippy::upper_case_acronyms)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OutputMode {
    PCM,
    DSD,
}

#[derive(Clone, Debug, sqlx::FromRow)]
pub struct Media {
    pub file_path: String,
    pub album: Album,
    pub name: String,
    pub artist: String,
    pub track: u8,
}

#[derive(Clone, Debug, sqlx::FromRow)]
pub struct Album {
    pub name: String,
    pub year: u16,
    pub track: u8,
    pub cover: String,
}

#[derive(Clone, Debug, sqlx::FromRow)]
pub struct AlbumInDb {
    pub id: i32,
    pub name: String,
    pub year: u16,
    pub track: u8,
    pub cover: String,
}

impl Default for AlbumInDb {
    fn default() -> Self {
        Self {
            id: DEFAULT_ALBUM_ID,
            name: DEFAULT_ALBUM_NAME.to_owned(),
            year: Default::default(),
            track: Default::default(),
            cover: Default::default(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct MediaSpec {
    pub sample_rate: u32,
    pub channel: u32,
    pub mode: OutputMode,
}

