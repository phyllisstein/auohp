//! Validate speaker diarization's turn-taking structure against a
//! hand-labeled reference.
//!
//! Skips Whisper --- this scores turn-taking structure, not transcript
//! content, so decoding audio once and diarizing it is the whole test;
//! paying for a ~25-minute large-v3 decode wouldn't change the score. The
//! actual scoring lives in [`auohp_core::eval::diarization`]; this binary is
//! just the audio -> diarize -> score pipeline around it, the same shape as
//! `score` around [`auohp_core::eval`]'s word-level scoring.
//!
//! Usage:
//!   cargo run --release --bin validate_diarization --features metal -- \
//!     --basename 108_diarization_test
//!
//! `--basename` resolves to `assets/<basename>.mp4` /
//! `assets/<basename>.mp4.txt`, so any interview with a hand-labeled
//! `HH:MM:SS:FF - HH:MM:SS:FF` / `Speaker N` reference can be scored the
//! same way.

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;

use auohp_core::eval::diarization::{embedding_diagnostic, parse_reference, score};
use auohp_core::transcription::{
    decode_file, diarize, dominant_speaker, extract_segment_embeddings, models_dir,
    EMBEDDING_MODEL_FILE, SEGMENTATION_MODEL_FILE,
};

#[derive(Parser, Debug)]
#[command(about = "Validate diarization turn-taking structure against a hand-labeled reference")]
struct Cli {
    /// Asset basename under `assets/`.
    #[arg(long, default_value = "026_original")]
    basename: String,

    /// Extract embeddings and report same-speaker vs. different-speaker
    /// distance separation instead of running clustering.
    #[arg(long)]
    embedding_diagnostic: bool,

    /// Print per-turn predictions and the first 40 predicted segments.
    #[arg(long)]
    verbose: bool,
}

fn main() -> Result<()> {
    tracing_subscriber::fmt().with_writer(std::io::stderr).init();
    let cli = Cli::parse();

    let audio_file = format!("{}.mp4", cli.basename);
    let reference_file = format!("{}.mp4.txt", cli.basename);

    let models_dir = models_dir();
    let asset_dir =
        PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string()))
            .join("../../assets");

    let reference_text = std::fs::read_to_string(asset_dir.join(&reference_file))
        .context("failed to read reference transcript")?;
    let reference = parse_reference(&reference_text)?;
    eprintln!("Reference: {} ground-truth turns", reference.len());

    eprintln!("Decoding audio...");
    let decoded = decode_file(&asset_dir.join(&audio_file))?;
    eprintln!(
        "Decoded {:.1}s of audio at {} Hz",
        decoded.samples.len() as f64 / decoded.sample_rate as f64,
        decoded.sample_rate
    );

    if cli.embedding_diagnostic {
        eprintln!("Extracting embeddings (diagnostic mode, no clustering)...");
        let embeddings = extract_segment_embeddings(
            &decoded.samples,
            decoded.sample_rate,
            &models_dir.join(SEGMENTATION_MODEL_FILE),
            &models_dir.join(EMBEDDING_MODEL_FILE),
        )?;
        let diag = embedding_diagnostic(&embeddings, &reference);

        println!("=== Embedding diagnostic ===");
        println!("Segments embedded: {}", diag.segments_embedded);
        println!(
            "Mean cosine distance, same speaker:      {:.4} (n={})",
            diag.same_speaker_mean_distance, diag.same_speaker_pairs
        );
        println!(
            "Mean cosine distance, different speaker: {:.4} (n={})",
            diag.diff_speaker_mean_distance, diag.diff_speaker_pairs
        );
        println!(
            "Embedding L2 norm: min={:.4} max={:.4} mean={:.4}",
            diag.norm_min, diag.norm_max, diag.norm_mean
        );
        return Ok(());
    }

    eprintln!("Running diarization...");
    let predicted = diarize(
        &decoded.samples,
        decoded.sample_rate,
        &models_dir.join(SEGMENTATION_MODEL_FILE),
        &models_dir.join(EMBEDDING_MODEL_FILE),
        2,
    )?;
    eprintln!("Predicted {} diarized segments", predicted.len());

    let result = score(&reference, &predicted);

    if cli.verbose {
        eprintln!("\n=== per-turn debug ===");
        for t in &reference {
            let pred = dominant_speaker(t.start, t.end, &predicted);
            eprintln!(
                "[{:>7.1}-{:>7.1}] truth={:<16} pred={:?}",
                t.start, t.end, t.speaker, pred
            );
        }
        eprintln!("\n=== predicted segment timeline (first 40) ===");
        let mut predicted_sorted = predicted.clone();
        predicted_sorted.sort_by(|a, b| a.start.total_cmp(&b.start));
        for p in predicted_sorted.iter().take(40) {
            eprintln!("[{:>7.1}-{:>7.1}] {}", p.start, p.end, p.speaker);
        }
    }

    println!("=== Diarization validation: {audio_file} ===");
    println!("Reference turns:        {}", result.total_turns);
    println!("Reference speakers:     {:?}", result.reference_speakers);
    println!("Predicted segments:     {}", predicted.len());
    println!("Predicted labels:       {:?}", result.predicted_labels);
    println!("Best label mapping:     {:?}", result.best_mapping);
    println!(
        "Dominant-speaker accuracy: {:.1}% ({} / {} turns)",
        result.dominant_speaker_accuracy * 100.0,
        result.matched_turns,
        result.total_turns
    );
    println!(
        "Boundary recall (±{:.0}s): {:.1}% ({} / {} reference speaker changes)",
        result.boundary_tolerance_secs,
        result.boundary_recall * 100.0,
        result.detected_changes,
        result.total_changes
    );

    Ok(())
}
