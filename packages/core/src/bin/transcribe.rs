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
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[derive(Parser, Debug)]
struct Cli {
    /// Result output path (defaults to stdout)
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// File to transcribe
    file: PathBuf,
}

fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "auohp_core=debug".into()))
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .init();

    let cli = Cli::parse();
    let result = transcription::run(&cli.file)?;
    let json = serde_json::to_string_pretty(&result)?;

    if let Some(output_path) = cli.output {
        let mut file = File::create(output_path)?;
        file.write_all(json.as_bytes())?;
    } else {
        println!("{}", json);
    }

    Ok(())
}
