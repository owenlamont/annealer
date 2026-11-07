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

mod cache;
mod camera;
mod contributors;
mod export;
mod history;
mod layout;
mod model;
mod scene;
mod theme;
mod timeline;

use std::path::{Path, PathBuf};

use clap::{CommandFactory, Parser, Subcommand};

#[derive(Parser)]
#[command(version, about = "Git history inspection and layout playback study")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Inspect local history as JSON; merge changes compare against the first parent.
    Inspect {
        repository: PathBuf,
        #[arg(long, default_value = "HEAD")]
        revision: String,
    },
    /// Play history in a loop; brackets change speed, Escape closes it.
    Preview {
        #[command(flatten)]
        source: Source,
    },
    /// Write numbered PNG frames and a logical-state manifest to a new directory.
    Export {
        directory: PathBuf,
        #[command(flatten)]
        source: Source,
        /// Frames to write; defaults to the whole playback.
        #[arg(long, value_parser = clap::value_parser!(u32).range(2..=1_000_000))]
        frames: Option<u32>,
        #[arg(long, default_value_t = 30, value_parser = clap::value_parser!(u16).range(1..=240))]
        fps: u16,
        #[arg(long, default_value_t = 0)]
        frame_delay_ms: u16,
    },
}

#[derive(clap::Args)]
struct Source {
    /// Local repository to play; a built-in synthetic history plays when omitted.
    #[arg(long)]
    repository: Option<PathBuf>,
    #[arg(long, default_value = "HEAD")]
    revision: String,
    /// Import playback history without reading or writing the user cache.
    #[arg(long)]
    no_cache: bool,
    /// Contributor TOML file; defaults to the Codestrand user configuration directory.
    #[arg(long)]
    contributors: Option<PathBuf>,
    /// Commits laid out and settled before playback begins.
    #[arg(long, default_value_t = 0)]
    start: usize,
    /// Commits to play after the start; defaults to the rest of the history.
    #[arg(long)]
    commits: Option<usize>,
    /// Playback seconds per calendar day.
    #[arg(long, default_value_t = 0.5, value_parser = positive_seconds)]
    seconds_per_day: f32,
    /// Maximum playback seconds between commits, compressing idle gaps.
    #[arg(long, default_value_t = 1.0, value_parser = positive_seconds)]
    max_idle_seconds: f32,
}

impl Source {
    fn events(
        &self,
        cache_root: Option<&Path>,
    ) -> Result<Vec<timeline::Event>, history::Error> {
        match &self.repository {
            Some(path) => {
                cache::load(path, &self.revision, cache_root.filter(|_| !self.no_cache))
            }
            None => Ok(timeline::synthetic()),
        }
    }

    fn load(&self) -> anyhow::Result<(model::Simulation, String)> {
        let mappings = contributors::load(self.contributors.as_deref())?;
        let events = self.events(cache::root().as_deref())?;
        let (events, title) = match &self.repository {
            Some(path) => {
                let name = path
                    .canonicalize()
                    .ok()
                    .and_then(|path| {
                        path.file_name()
                            .map(|name| name.to_string_lossy().into_owned())
                    })
                    .unwrap_or_else(|| path.display().to_string());
                (events, name)
            }
            None => (events, "Synthetic history".to_owned()),
        };
        let options = model::Options {
            seconds_per_day: self.seconds_per_day,
            max_idle_seconds: self.max_idle_seconds,
            start: self.start,
            commits: self.commits,
        };
        Ok((
            model::Simulation::new(events, &options, &theme::Theme::neon(), &mappings),
            title,
        ))
    }
}

fn positive_seconds(value: &str) -> Result<f32, String> {
    match value.parse::<f32>() {
        Ok(seconds) if (0.01..=86_400.0).contains(&seconds) => Ok(seconds),
        _ => Err("expected seconds between 0.01 and 86400".to_owned()),
    }
}

fn main() -> anyhow::Result<()> {
    match Cli::parse().command {
        Some(Command::Inspect {
            repository,
            revision,
        }) => {
            let history = history::import(&repository, &revision)?;
            serde_json::to_writer_pretty(std::io::stdout().lock(), &history)?;
        }
        Some(Command::Preview { source }) => {
            let (simulation, title) = source.load()?;
            scene::preview(simulation, title);
        }
        Some(Command::Export {
            directory,
            source,
            frames,
            fps,
            frame_delay_ms,
        }) => {
            export::prepare(&directory)?;
            let (simulation, title) = source.load().inspect_err(|_| {
                let _ = std::fs::remove_dir(&directory);
            })?;
            let frames =
                frames.unwrap_or_else(|| model::frames_for(simulation.duration(), fps));
            export::run(&directory, simulation, title, frames, fps, frame_delay_ms)?;
        }
        None => Cli::command().print_help()?,
    }
    Ok(())
}
