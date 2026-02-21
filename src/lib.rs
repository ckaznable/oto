pub mod alloc;
pub mod cli;
pub mod config;
pub mod decoder;
pub mod devices;
pub mod event;
pub mod media;
pub mod mpris;
pub mod player;
pub mod shared;
pub mod tui;
pub mod util;
pub mod volume;

#[cfg(feature = "dict-jp")]
pub mod dict;
