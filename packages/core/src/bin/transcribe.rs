//! Standalone transcription binary for acceptance testing and manual runs.
//!
//! Usage:
//!   cargo run --bin transcribe -- <input_file>
//!
//! Writes the TranscriptionResult JSON to stdout; logs go to stderr.
//! Redirect as needed:
//!   cargo run --bin transcribe -- clip.mp4 > out.json 2> out.log

#[path = "../transcription/mod.rs"]
mod transcription;

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;
use std::{fs::File, io::Write};
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Parser, Debug)]
struct Cli {
    /// Result output path (defaults to stdout)
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// TranscribeConfig JSON. Defaults to the baseline configuration.
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Write the effective config here, including defaults that were not
    /// specified. The run manifest should record this, not the input config ---
    /// they differ whenever a knob was left unset, and the manifest has to
    /// describe what actually ran.
    #[arg(long)]
    dump_config: Option<PathBuf>,

    /// File to transcribe
    file: PathBuf,
}

fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "auohp_core=debug".into()))
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .init();

    let cli = Cli::parse();

    let config = match &cli.config {
        Some(p) => transcription::TranscribeConfig::from_json(&std::fs::read_to_string(p)?)?,
        None => transcription::TranscribeConfig::default(),
    };
    if let Some(p) = &cli.dump_config {
        std::fs::write(p, serde_json::to_string_pretty(&config)?)?;
    }

    let result = transcription::run_with(&cli.file, &config)?;
    let json = serde_json::to_string_pretty(&result)?;

    if let Some(output_path) = cli.output {
        let mut file = File::create(output_path)?;
        file.write_all(json.as_bytes())?;
    } else {
        println!("{}", json);
    }

    Ok(())
}
