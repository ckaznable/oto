use anyhow::Result;
use id3::TagLike;
use lofty::{
    config::ParseOptions,
    file::{AudioFile, TaggedFileExt},
    probe::Probe,
    tag::Accessor,
};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use std::{
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
    rc::Rc,
};
use walkdir::WalkDir;

use bitcode::{Decode, Encode};

use crate::{decoder::DsfReader, shared::PROJ_DIRS, util::cover_from_path};

pub type Tracks = Vec<TrackMeta>;
/// artist, ( album name, indexes of playlist )
/// or album name, ( artist, indexes of playlist )
pub type TracksTree = Vec<(Rc<String>, Vec<(Rc<String>, Vec<usize>)>)>;

#[allow(clippy::upper_case_acronyms)]
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Encode, Decode)]
pub enum OutputMode {
    #[default]
    PCM,
    DSD,
}

#[derive(Clone, Default, Debug, Encode, Decode, PartialEq, Eq)]
pub struct Album {
    pub name: Option<String>,
    pub year: Option<u32>,
    pub track: Option<u32>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Default, Encode, Decode)]
pub struct MediaSpec {
    pub sample_rate: u32,
    pub duration: Option<u64>,
    pub channels: u32,
    pub mode: OutputMode,
}

#[derive(Clone, Default, Debug, Encode, Decode)]
pub struct TrackMeta {
    pub path: String,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Album,
    pub duration_secs: u64,
}

impl TrackMeta {
    pub fn path(&self) -> PathBuf {
        self.path.clone().into()
    }

    pub fn empty(path: String) -> Self {
        Self {
            path,
            ..Default::default()
        }
    }

    pub fn cover(&self) -> Option<Vec<u8>> {
        cover_from_path(&self.path())
    }

    pub fn is_album_same(&self, other: &Self) -> bool {
        self.artist == other.artist
            && self.album.name == other.album.name
            && self.album.year == other.album.year
    }
}

pub struct MediaStore;

impl MediaStore {
    pub fn get_tracks(path: Option<&Path>) -> Tracks {
        match path {
            Some(p) => {
                let mut list = vec![];
                if !p.exists() {
                    return list;
                }

                if p.is_file()
                    && let Some(track) = Self::parse_one_file(p)
                {
                    list.push(track);
                } else {
                    list = Self::scan_tracks(p);
                }

                Self::save_tracks(&list).ok();
                list
            }
            None => Self::load_tracks().ok().unwrap_or_default(),
        }
    }

    pub fn get_tracks_tree(tracks: &[TrackMeta]) -> (TracksTree, TracksTree) {
        use std::collections::HashMap;

        let mut artist_map: HashMap<Rc<String>, HashMap<Rc<String>, Vec<usize>>> = HashMap::new();
        let mut album_map: HashMap<Rc<String>, HashMap<Rc<String>, Vec<usize>>> = HashMap::new();

        for (idx, track) in tracks.iter().enumerate() {
            let artist = Rc::new(
                track
                    .artist
                    .as_ref()
                    .cloned()
                    .unwrap_or_else(|| "Unknown".to_string()),
            );
            let album_name = Rc::new(
                track
                    .album
                    .name
                    .as_ref()
                    .cloned()
                    .unwrap_or_else(|| "Unknown".to_string()),
            );

            artist_map
                .entry(artist.clone())
                .or_default()
                .entry(album_name.clone())
                .or_default()
                .push(idx);

            album_map
                .entry(album_name)
                .or_default()
                .entry(artist)
                .or_default()
                .push(idx);
        }

        (
            artist_map
                .into_iter()
                .map(|(artist, albums)| (artist, albums.into_iter().collect()))
                .collect(),
            album_map
                .into_iter()
                .map(|(albums, artist)| (albums, artist.into_iter().collect()))
                .collect(),
        )
    }

    pub fn tracks_file_path() -> PathBuf {
        PROJ_DIRS.data_dir().join("tracks")
    }

    pub fn save_tracks(tracks: &Tracks) -> Result<()> {
        let path = Self::tracks_file_path();
        std::fs::create_dir_all(path.parent().unwrap())?;

        let raw_bytes = bitcode::encode(tracks);

        let file = File::create(path)?;
        let mut encoder = zstd::stream::Encoder::new(file, 3)?;
        encoder.write_all(&raw_bytes)?;
        encoder.finish()?;
        Ok(())
    }

    pub fn load_tracks() -> Result<Tracks> {
        let path = Self::tracks_file_path();
        let file = File::open(path)?;
        let mut decoder = zstd::stream::Decoder::new(file)?;
        let mut buffer = Vec::new();
        decoder.read_to_end(&mut buffer)?;

        let tracks: Tracks = bitcode::decode(&buffer)?;
        Ok(tracks)
    }

    pub fn scan_tracks(root_path: &Path) -> Vec<TrackMeta> {
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
                Self::parse_one_file(path)
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
            path: path.to_string_lossy().to_string(),
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
            return Self::parse_dsf_file(path);
        }

        let options = ParseOptions::new().read_cover_art(false);
        let tagged_file = Probe::open(path).ok()?.options(options).read().ok()?;

        let tag = tagged_file
            .primary_tag()
            .or_else(|| tagged_file.first_tag())?;

        let properties = tagged_file.properties();

        Some(TrackMeta {
            path: path.to_string_lossy().to_string(),
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
}
