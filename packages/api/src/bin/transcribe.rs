//! Standalone transcription binary for acceptance testing and manual runs.
//!
//! Usage:
//!   cargo run --bin transcribe -- <input_file> [models_dir]
//!
//! Writes the TranscriptionResult JSON to stdout; logs go to stderr.
//! Redirect as needed:
//!   cargo run --bin transcribe -- clip.mp4 > out.json 2> out.log

// Re-use the pipeline source files directly. The #[path] attribute tells
// the compiler to look for the module root at a specific path relative to
// this file, rather than the default `src/bin/transcription/mod.rs`.
#[path = "../transcription/mod.rs"]
mod transcription;

use anyhow::{Context, Result};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "auohp_api=debug".into()))
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .init();

    let mut args = std::env::args().skip(1);
    let input = args
        .next()
        .context("usage: transcribe <input_file> [models_dir]")?;
    let models_dir = args.next().unwrap_or_else(|| "models".to_string());

    let config =
        transcription::PipelineConfig::from_model_dir(std::path::Path::new(&models_dir), 2);

    let result = transcription::run(&config, std::path::Path::new(&input))?;
    println!("{}", serde_json::to_string_pretty(&result)?);

    Ok(())
}
