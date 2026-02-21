use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(version, long_about = None)]
pub struct CliArgs {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    #[cfg(feature = "dict-jp")]
    Init,
    Play(CommonArgs),
    Tui(CommonArgs),
}

#[derive(Args, Debug)]
pub struct CommonArgs {
    #[arg(short, long)]
    pub path: Option<PathBuf>,

    #[arg(short, long)]
    pub device: Option<String>,
}

#[derive(Subcommand, Debug)]
pub enum PlayListCommands {
    Init,
    Refresh,
}
