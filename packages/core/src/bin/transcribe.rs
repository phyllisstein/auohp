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
use chrono::Utc;
use clap::Parser;
use serde_json::json;
use std::{fs::File, io::Write, path::PathBuf, process::Command};
use tokio::task;
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

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "auohp_core=warn".into()))
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
                println!("git rev-parse failed: {:?}", err);
                "#######".into()
            }
        }
        Err(e) => {
            println!("git rev-parse failed: {:?}", e);
            "#######".into()
        }
    }
    .trim()
    .into();

    let run_start = Utc::now();

    let cli = Cli::parse();

    let config = match &cli.config {
        Some(p) => transcription::TranscribeConfig::from_json(&std::fs::read_to_string(p)?)?,
        None => transcription::TranscribeConfig::default(),
    };

    let input_path = cli.file.canonicalize()?;
    let pths = if input_path.is_file() {
        vec![input_path.clone()]
    } else {
        input_path
            .read_dir()?
            .map(|rd| rd.unwrap().path())
            .filter(|pth| pth.extension().unwrap() == "mp4")
            .collect()
    };

    for p in pths {
        let p_cfg = config.clone();
        let p_hsh = git_hash.clone();

        let mut p_fn = p.clone().with_added_extension("json");
        if let Some(opth) = &cli.output {
            p_fn = opth.join(p_fn.file_name().unwrap());
        };
        let p_label = p_fn.file_name().unwrap();

        if p_fn.exists() {
            println!(
                "[{:?}] {:?}\tskipping ({:?} exists)",
                Utc::now(),
                p_label,
                p_fn
            );
            continue;
        }

        println!("[{:?}] {:?}\ttranscribing", Utc::now(), p_label);
        let transcription_output = task::spawn_blocking(move || {
            let transcription = transcription::run_with(&p, &p_cfg).unwrap();
            let results = json!({
                "transcription": transcription,
                "run": {
                    "config": p_cfg,
                    "gitHash": p_hsh,
                    "start": run_start,
                    "source": &p.file_name().unwrap().to_string_lossy(),
                },
            });

            serde_json::to_string_pretty(&results).unwrap()
        })
        .await?;
        println!("[{:?}] {:?}\tsaving", Utc::now(), p_label);

        let mut file = File::create(p_fn)?;
        file.write_all(transcription_output.as_bytes())?;
    }

    Ok(())
}
