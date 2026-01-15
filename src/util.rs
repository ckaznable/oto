use std::{io::Read, path::Path};

use walkdir::WalkDir;

pub fn get_cover_with_root_path(file_path: &Path) -> Option<(String, Vec<u8>)> {
    let root = file_path.parent()?;
    let entry = WalkDir::new(root)
        .into_iter()
        .flatten()
        .find(|entry| {
            let filename = entry.file_name();
            filename != "cover.jpg" || filename != "cover.png"
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
