//! Standalone transcription binary for acceptance testing and manual runs.
//!
//! Usage:
//!   cargo run --bin transcribe -- <input_file> [models_dir]
//!
//! models_dir defaults to $MODELS_DIR, then "./models" if neither is set.
//!
//! Writes the TranscriptionResult JSON to stdout; logs go to stderr.
//! Redirect as needed:
//!   cargo run --bin transcribe -- clip.mp4 > out.json 2> out.log

// Re-use the pipeline source files directly. The #[path] attribute tells
// the compiler to look for the module root at a specific path relative to
// this file, rather than the default `src/bin/transcription/mod.rs`.
#[path = "../transcription/mod.rs"]
mod transcription;

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;
use std::{fs::File, io::Write};
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Parser, Debug)]
struct Cli {
    /// Maximum number of speakers for diarization
    #[arg(short, long, default_value_t = 3)]
    speakers: usize,

    /// Path to ML models
    #[arg(short, long)]
    models: Option<PathBuf>,

    /// Result output path
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// File to transcribe
    file: PathBuf,
}

fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "auohp_api=debug".into()))
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .init();

    let cli = Cli::parse();

    let config = if let Some(models_dir) = cli.models {
        transcription::PipelineConfig::from_model_dir(&models_dir, cli.speakers)
    } else {
        transcription::PipelineConfig::from_env(cli.speakers)
    };

    let result = transcription::run(&config, &cli.file)?;
    let json = serde_json::to_string_pretty(&result)?;

    if let Some(output_path) = cli.output {
        let mut file = File::create(output_path)?;
        file.write_all(json.as_bytes())?;
    } else {
        println!("{}", json);
    }

    Ok(())
}
