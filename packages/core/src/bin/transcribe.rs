//! Standalone transcription binary for acceptance testing and manual runs.
//!
//! Usage:
//!   cargo run --bin transcribe -- <input_file>
//!
//! Writes the TranscriptionResult JSON to stdout; logs go to stderr.
//! Redirect as needed:
//!   cargo run --bin transcribe -- clip.mp4 > out.json 2> out.log

use anyhow::Result;
use auohp_core::transcription::{self, TranscribeConfig, TranscriptionResult};
use clap::Parser;
use serde::Serialize;
use std::{fs::File, io::Write, path::PathBuf, process::Command};
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Parser, Debug)]
#[clap(name = "transcribe")]
struct Cli {
    /// Directory for saving transcription JSON. Default: the input path.
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// TranscribeConfig JSON. Defaults to the baseline configuration.
    #[arg(long)]
    config: Option<PathBuf>,

    /// Source audio/video file, or a directory containing multiple sources
    file: PathBuf,
}

#[derive(Serialize)]
struct ResultsWithConfig {
    git_hash: String,
    transcription: TranscriptionResult,
    config: TranscribeConfig,
}

fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "auohp_core=debug".into()))
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .init();

    let git_hash: String = match Command::new("git")
        .arg("rev-parse")
        .arg("--short")
        .arg("HEAD")
        .output()
    {
        Ok(o) => {
            if o.status.success() {
                String::from_utf8_lossy(&o.stdout).into_owned()
            } else {
                let err = String::from_utf8_lossy(&o.stderr).into_owned();
                tracing::error!(err, "git rev-parse failed");
                "#######".into()
            }
        }
        Err(e) => {
            tracing::error!(err = e.to_string(), "git rev-parse failed");
            "#######".into()
        }
    }
    .trim()
    .into();

    let cli = Cli::parse();

    let config = match &cli.config {
        Some(p) => transcription::TranscribeConfig::from_json(&std::fs::read_to_string(p)?)?,
        None => transcription::TranscribeConfig::default(),
    };

    if cli.file.is_file() {
        let transcription = transcription::run_with(&cli.file, &config)?;
        let config = config.clone();
        let git_hash = git_hash.clone();
        let results = ResultsWithConfig {
            transcription,
            config,
            git_hash,
        };
        let json = serde_json::to_string_pretty(&results)?;

        if let Some(mut output_path) = cli.output.clone() {
            let fname = cli.file.file_name().unwrap();
            output_path.push(fname);
            output_path.set_extension("json");

            let mut file = File::create(output_path)?;
            file.write_all(json.as_bytes())?;
        } else {
            let mut fname = cli.file.clone();
            fname.set_extension("json");

            let mut file = File::create(fname)?;
            file.write_all(json.as_bytes())?;
        }
    }

    if cli.file.is_dir() {
        for f in cli.file.read_dir()? {
            let f = f?.path();
            if f.is_file() {
                let config = config.clone();
                let git_hash = git_hash.clone();
                let transcription = transcription::run_with(&f, &config)?;
                let results = ResultsWithConfig {
                    transcription,
                    config,
                    git_hash,
                };
                let json = serde_json::to_string_pretty(&results)?;

                if let Some(mut output_path) = cli.output.clone() {
                    let fname = cli.file.file_name().unwrap();
                    output_path.push(fname);
                    output_path.set_extension("json");

                    let mut file = File::create(output_path)?;
                    file.write_all(json.as_bytes())?;
                } else {
                    let mut fname = cli.file.clone();
                    fname.set_extension("json");

                    let mut file = File::create(fname)?;
                    file.write_all(json.as_bytes())?;
                }
            }
        }
    }

    Ok(())
}
