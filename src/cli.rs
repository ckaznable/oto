use std::path::PathBuf;

use clap::{command, Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(version, long_about = None)]
pub struct Args {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    Play {
        #[arg(short, long)]
        path: PathBuf,

        #[arg(short, long)]
        device: String,
    },

    PlayList {
        #[command(subcommand)]
        command: PlayListCommands,
    }
}

#[derive(Subcommand, Debug)]
pub enum PlayListCommands {
    Init,
    Refresh,
}
