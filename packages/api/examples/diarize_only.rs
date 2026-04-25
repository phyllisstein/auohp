//! Isolation harness: decode a WAV with our `audio.rs` pipeline, hand the
//! resulting i16 buffer to pyannote-rs, and report what comes back.  No
//! Whisper, no aligner, no merge step.  Lets us tell whether pyannote-rs is
//! returning segments at all on this audio + this model file.
//!
//! Run:  cargo run --example diarize_only -- <input.wav> [models_dir]
//!
//! `models_dir` defaults to `$MODELS_DIR`, then `./models`.

#[path = "../src/transcription/audio.rs"]
mod audio;
#[path = "../src/transcription/diarize.rs"]
mod diarize;

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
struct Cli {
    /// Linear gain applied to f32 samples before i16 conversion.
    /// 1.0 = unchanged; 0.5 = -6 dB; 2.0 = +6 dB (will clip at i16 limits).
    #[arg(short, long, default_value_t = 1.0)]
    gain: f32,

    /// Directory containing pyannote-segmentation-3.0.onnx and the wespeaker
    /// embedding model. Falls back to $MODELS_DIR, then "./models".
    #[arg(short, long)]
    models: Option<PathBuf>,

    /// WAV/audio file to diarize.
    file: PathBuf,
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "diarize_only=debug".into()),
        )
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();
    let input = cli.file;
    let models_dir: PathBuf = cli
        .models
        .or_else(|| std::env::var("MODELS_DIR").ok().map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("models"));

    let seg_model = models_dir.join("pyannote-segmentation-3.0.onnx");
    let emb_model = models_dir.join("wespeaker_en_voxceleb_ECAPA1024.onnx");

    eprintln!("decoding {}", input.display());
    let decoded = audio::decode_file(&input)?;
    eprintln!(
        "decoded: {} samples @ {} Hz ({:.1}s)",
        decoded.samples.len(),
        decoded.sample_rate,
        decoded.samples.len() as f64 / decoded.sample_rate as f64
    );

    // Apply linear gain in f32 before quantizing to i16 so we have headroom
    // and a single (cheap, branch-free) clamp at the integer boundary.
    let scaled: Vec<f32> = if (cli.gain - 1.0).abs() < f32::EPSILON {
        decoded.samples.clone()
    } else {
        eprintln!("applying gain: {:.3}x ({:+.2} dB)", cli.gain, 20.0 * cli.gain.log10());
        decoded.samples.iter().map(|s| s * cli.gain).collect()
    };
    let samples_i16 = diarize::f32_to_i16(&scaled);
    eprintln!("calling pyannote_rs::get_segments directly");
    let raw: Vec<_> = pyannote_rs::get_segments(
        &samples_i16,
        decoded.sample_rate,
        seg_model.to_str().expect("seg path utf-8"),
    )
    .map_err(|e| anyhow::anyhow!("get_segments failed: {e}"))?
    .collect();

    let total = raw.len();
    let ok = raw.iter().filter(|r| r.is_ok()).count();
    let err = total - ok;
    eprintln!("pyannote raw: {total} items ({ok} ok, {err} err)");
    for (i, r) in raw.iter().enumerate().take(5) {
        match r {
            Ok(seg) => eprintln!("  [{i}] ok: {:.2}–{:.2}s ({} samples)", seg.start, seg.end, seg.samples.len()),
            Err(e) => eprintln!("  [{i}] err: {e}"),
        }
    }

    eprintln!("now running full diarize() (segmentation + embedding + clustering)");
    let diarized = diarize::diarize(&samples_i16, decoded.sample_rate, &seg_model, &emb_model, 3)?;
    eprintln!("final diarized count: {}", diarized.len());

    Ok(())
}
