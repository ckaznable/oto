use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(version, long_about = None)]
pub struct Args {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    #[cfg(feature = "dict-jp")]
    Init,
    Play {
        #[arg(short, long)]
        path: Option<PathBuf>,

        #[arg(short, long)]
        device: Option<String>,
    },

    Tui {
        #[arg(short, long)]
        path: Option<PathBuf>,

        #[arg(short, long)]
        device: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
pub enum PlayListCommands {
    Init,
    Refresh,
}
