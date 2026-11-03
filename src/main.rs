#![forbid(unsafe_code)]
#![deny(
    clippy::all,
    clippy::pedantic,
    clippy::cargo,
    clippy::cognitive_complexity,
    clippy::allow_attributes,
    clippy::allow_attributes_without_reason
)]
#![expect(
    clippy::multiple_crate_versions,
    reason = "Bevy's platform backends require incompatible transitive crate versions"
)]

mod export;
mod model;
mod scene;

use std::path::PathBuf;

use clap::{CommandFactory, Parser, Subcommand};

#[derive(Parser)]
#[command(version, about = "Annealer rendering feasibility probe")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    Preview,
    Export {
        directory: PathBuf,
        #[arg(long, default_value_t = 120, value_parser = clap::value_parser!(u16).range(2..=3600))]
        frames: u16,
        #[arg(long, default_value_t = 30, value_parser = clap::value_parser!(u16).range(1..=240))]
        fps: u16,
        #[arg(long, default_value_t = 0)]
        frame_delay_ms: u16,
    },
}

fn main() -> anyhow::Result<()> {
    match Cli::parse().command {
        Some(Command::Preview) => scene::preview(),
        Some(Command::Export {
            directory,
            frames,
            fps,
            frame_delay_ms,
        }) => export::run(&directory, frames, fps, frame_delay_ms)?,
        None => Cli::command().print_help()?,
    }
    Ok(())
}
