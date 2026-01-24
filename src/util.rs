use std::{io::Read, path::Path};

use lofty::{config::ParseOptions, file::TaggedFileExt, probe::Probe};
use walkdir::WalkDir;

use crate::decoder::DsfReader;

pub fn get_cover_with_root_path(file_path: &Path) -> Option<(String, Vec<u8>)> {
    let root = file_path.parent()?;
    let entry = WalkDir::new(root)
        .into_iter()
        .flatten()
        .find(|entry| {
            let filename = entry.file_name();
            filename == "cover.jpg" || filename == "cover.png"
        })
        .map(|entry| entry.path().to_owned())?;

    let mime = if entry.extension()?.to_str()? == "jpg" {
        "image/jpg"
    } else {
        "image/png"
    };

    let mut data = Vec::new();
    let mut file = std::fs::File::open(entry).ok()?;
    file.read_to_end(&mut data).ok()?;
    Some((mime.to_owned(), data))
}

pub fn cover_from_path(path: &Path) -> Option<Vec<u8>> {
    if let Some(ext) = path.extension()
        && ext == "dsf"
    {
        return cover_dsf(path);
    }

    let options = ParseOptions::new().read_properties(false);
    let tagged_file = Probe::open(path).ok()?.options(options).read().ok()?;
    let tag = tagged_file
        .primary_tag()
        .or_else(|| tagged_file.first_tag())?;

    tag.pictures()
        .first()
        .map(|pic| pic.data().to_vec())
        .or_else(|| Some(get_cover_with_root_path(path)?.1))
}

pub fn cover_dsf(path: &Path) -> Option<Vec<u8>> {
    let mut file = std::fs::File::open(path).ok()?;
    let reader = DsfReader::new(&mut file);
    let metadata = reader.parse().ok()?;
    metadata.tag?.pictures().next().map(|p| p.data.clone())
}
