use std::sync::LazyLock;

use directories::ProjectDirs;

const I32_BYTE: usize = i32::BITS as usize / 8;

// 256kb i32
pub const TMP_BUF_ALLOC: usize = (1024 * 256) / I32_BYTE;

// 1mb i32
pub const RING_BUF_ALLOC: usize = (1024 * 1024) / I32_BYTE;

pub static PROJ_DIRS: LazyLock<ProjectDirs> =
    LazyLock::new(|| ProjectDirs::from("", "", "oto").unwrap());
