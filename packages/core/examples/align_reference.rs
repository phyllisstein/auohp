//! QC demonstration for restored wav2vec2 forced alignment.
//!
//! Force-aligns a hand-picked line from this project's own human-transcribed
//! validation reference (`assets/026_original.mp4.txt`) against the matching
//! span of `assets/026_original.mp4` --- text neither Whisper nor DTW ever
//! saw, which is exactly the restored role documented in
//! `transcription/align.rs`: timestamping externally supplied transcript
//! text, not refining Whisper's own.
//!
//! Usage:
//!   cargo run --example align_reference --features metal

use anyhow::{Context, Result};
use auohp_core::transcription::{decode_file, Aligner};
use std::path::PathBuf;

fn main() -> Result<()> {
    tracing_subscriber::fmt().with_writer(std::io::stderr).init();

    let models_dir = PathBuf::from(
        std::env::var("MODELS_DIR").unwrap_or_else(|_| "/opt/auohp/models".to_string()),
    );
    let asset_dir = PathBuf::from(
        std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string()),
    )
    .join("../../assets");

    // Reference turn 3 (see assets/026_original.mp4.txt): 00:01:05:03 -
    // 00:01:13:21, Sarah Schulman. Picked because it's short enough to
    // align quickly and long enough (>8s) to be a meaningful CTC run.
    let start = 65.1;
    let end = 73.7;
    let text = "So when you say you brought this information about the trials to act up, \
                did you just stand up in the middle of the room and tell everyone, or did \
                you go to somebody.";

    eprintln!("Decoding audio...");
    let decoded = decode_file(&asset_dir.join("026_original.mp4"))?;
    let start_sample = (start * decoded.sample_rate as f64) as usize;
    let end_sample = ((end * decoded.sample_rate as f64) as usize).min(decoded.samples.len());
    let audio_slice = &decoded.samples[start_sample..end_sample];

    eprintln!("Loading wav2vec2...");
    let mut aligner = Aligner::load(&models_dir.join("wav2vec2-base-960h-quantized.onnx"))
        .context("failed to load aligner")?;

    eprintln!("Aligning {} known words against {:.1}s of audio...", text.split_whitespace().count(), end - start);
    let words = aligner.align(audio_slice, text, start)?;

    println!("=== Forced alignment: reference turn 3 (Sarah Schulman, {start:.1}-{end:.1}s) ===");
    for w in &words {
        println!("[{:>6.2}-{:>6.2}] {}", w.start, w.end, w.word);
    }

    anyhow::ensure!(!words.is_empty(), "alignment produced no words");
    anyhow::ensure!(
        words.iter().all(|w| w.start >= start - 0.1 && w.end <= end + 0.1),
        "aligned timestamps fell outside the known audio span"
    );
    anyhow::ensure!(
        words.windows(2).all(|w| w[0].start <= w[1].start),
        "aligned words are not in chronological order"
    );
    eprintln!("OK: {} words aligned, monotonic, within span.", words.len());

    Ok(())
}
