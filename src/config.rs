use anyhow::{anyhow, Result};
use std::{io::Read, path::PathBuf};

use serde::{Deserialize, Serialize};

use crate::shared::PROJ_DIRS;

#[derive(Serialize, Deserialize, Default)]
pub struct AppConfig {
    pub path: Option<PathBuf>,
    pub device: Option<String>,
    pub device_name: Option<String>,
}

impl AppConfig {
    pub fn new() -> Self {
        Self::read_from_file().unwrap_or_default()
    }

    pub fn read_from_file() -> Result<Self> {
        let p = PROJ_DIRS.config_local_dir().join("config.toml");
        if !p.exists() {
            return Err(anyhow!("config not found"));
        }

        let mut f = std::fs::File::open(p)?;
        let mut buf = vec![];
        f.read_to_end(&mut buf)?;
        Ok(toml::from_slice(&buf)?)
    }
}
