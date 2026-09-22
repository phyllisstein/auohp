//! Turn-detection validation harness for restored speaker diarization.
//!
//! Runs `transcription::diarize` directly against `assets/026_original.mp4`
//! (skipping Whisper --- this harness scores *turn-taking structure*, not
//! transcript content, so decoding audio once and diarizing it is the whole
//! test; paying for a ~25-minute large-v3 decode wouldn't change the score)
//! and compares the result against the hand-transcribed, accurately
//! diarized reference at `assets/026_original.mp4.txt`.
//!
//! Usage:
//!   cargo run --example diarize_validate --features metal
//!
//! ## Metric
//!
//! The reference gives ground-truth speaker turns as timecode blocks. This
//! harness does not attempt word-level or DER-style scoring (explicitly out
//! of scope --- see the task brief); instead it asks two turn-taking
//! questions per ground-truth turn:
//!
//!   1. **Dominant-speaker accuracy**: does the predicted speaker label
//!      holding the most of this turn's time span --- summed across all that
//!      speaker's overlapping segments, via `transcription::dominant_speaker`,
//!      the same function the pipeline labels segments with --- match the
//!      reference speaker, after mapping predicted labels to reference names
//!      by whichever assignment maximizes agreement?
//!   2. **Boundary accuracy**: at each reference speaker change, does the
//!      predicted diarization also change speaker within a tolerance window?
//!
//! Both are reported; neither requires exact frame parity with the
//! reference, which a human editor cut by hand and which this harness
//! assumes is ~30 fps for its `HH:MM:SS:FF` timecodes (the tolerance below
//! absorbs any frame-rate error).

use std::path::PathBuf;

use anyhow::{Context, Result};
use auohp_core::transcription::{
    cosine_distance, decode_file, diarize, dominant_speaker, extract_segment_embeddings,
    models_dir, EMBEDDING_MODEL_FILE, SEGMENTATION_MODEL_FILE,
};

const REFERENCE_FPS: f64 = 30.0;
/// How close a predicted speaker change must land to a ground-truth change
/// to count as detected. Generous on purpose: a human editor's cut point and
/// a clustering algorithm's turn boundary will never coincide to the frame,
/// and the brief explicitly asks for turn detection, not DER parity.
const BOUNDARY_TOLERANCE_SECS: f64 = 3.0;

struct ReferenceTurn {
    start: f64,
    end: f64,
    speaker: String,
}

fn parse_timecode(tc: &str) -> Result<f64> {
    let parts: Vec<&str> = tc.split(':').collect();
    anyhow::ensure!(parts.len() == 4, "expected HH:MM:SS:FF, got {tc}");
    let h: f64 = parts[0].parse()?;
    let m: f64 = parts[1].parse()?;
    let s: f64 = parts[2].parse()?;
    let f: f64 = parts[3].parse()?;
    Ok(h * 3600.0 + m * 60.0 + s + f / REFERENCE_FPS)
}

fn parse_reference(text: &str) -> Result<Vec<ReferenceTurn>> {
    let mut turns = Vec::new();
    let mut lines = text.lines().peekable();

    while let Some(line) = lines.next() {
        let line = line.trim();
        if line.is_empty() || line.parse::<u64>().is_ok() {
            continue; // blank separator or block index
        }
        // This line is the timecode range: "HH:MM:SS:FF - HH:MM:SS:FF".
        let (start_tc, end_tc) = line
            .split_once(" - ")
            .with_context(|| format!("expected timecode range, got: {line}"))?;
        let start = parse_timecode(start_tc.trim())?;
        let end = parse_timecode(end_tc.trim())?;

        let speaker = lines
            .next()
            .context("expected speaker name after timecode line")?
            .trim()
            .to_string();

        // Consume (and ignore) the transcript text line(s) up to the next
        // blank line --- turn detection only needs the ground-truth speaker
        // change points, not the words.
        for text_line in lines.by_ref() {
            if text_line.trim().is_empty() {
                break;
            }
        }

        turns.push(ReferenceTurn { start, end, speaker });
    }

    Ok(turns)
}

fn main() -> Result<()> {
    tracing_subscriber::fmt().with_writer(std::io::stderr).init();

    let models_dir = models_dir();
    let asset_dir = PathBuf::from(
        std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string()),
    )
    .join("../../assets");

    let reference_text = std::fs::read_to_string(asset_dir.join("026_original.mp4.txt"))
        .context("failed to read reference transcript")?;
    let reference = parse_reference(&reference_text)?;
    eprintln!("Reference: {} ground-truth turns", reference.len());

    eprintln!("Decoding audio...");
    let decoded = decode_file(&asset_dir.join("026_original.mp4"))?;
    eprintln!(
        "Decoded {:.1}s of audio at {} Hz",
        decoded.samples.len() as f64 / decoded.sample_rate as f64,
        decoded.sample_rate
    );

    if std::env::var("EMBEDDING_DIAGNOSTIC").is_ok() {
        eprintln!("Extracting embeddings (diagnostic mode, no clustering)...");
        let embeddings = extract_segment_embeddings(
            &decoded.samples,
            decoded.sample_rate,
            &models_dir.join(SEGMENTATION_MODEL_FILE),
            &models_dir.join(EMBEDDING_MODEL_FILE),
        )?;

        // Label each embedded segment by whichever reference speaker covers
        // most of its span, then compare mean cosine distance for
        // same-speaker pairs against different-speaker pairs. If the model
        // and features carry any discriminative signal at all, same-speaker
        // pairs should sit measurably closer than cross-speaker pairs; if
        // the two numbers come out indistinguishable, the embeddings
        // themselves --- not the clustering cutoff --- are the problem.
        let labeled: Vec<(&str, &[f32])> = embeddings
            .iter()
            .filter_map(|seg| {
                let mid_turn = reference.iter().max_by(|a, b| {
                    let overlap = |t: &ReferenceTurn| {
                        (seg.end.min(t.end) - seg.start.max(t.start)).max(0.0)
                    };
                    overlap(a).total_cmp(&overlap(b))
                })?;
                Some((mid_turn.speaker.as_str(), seg.embedding.as_slice()))
            })
            .collect();

        let mut same_sum = 0.0f64;
        let mut same_n = 0u64;
        let mut diff_sum = 0.0f64;
        let mut diff_n = 0u64;
        for i in 0..labeled.len() {
            for j in (i + 1)..labeled.len() {
                let d = cosine_distance(labeled[i].1, labeled[j].1);
                if labeled[i].0 == labeled[j].0 {
                    same_sum += d;
                    same_n += 1;
                } else {
                    diff_sum += d;
                    diff_n += 1;
                }
            }
        }
        println!("=== Embedding diagnostic ===");
        println!("Segments embedded: {}", embeddings.len());
        println!(
            "Mean cosine distance, same speaker:      {:.4} (n={})",
            same_sum / same_n.max(1) as f64,
            same_n
        );
        println!(
            "Mean cosine distance, different speaker: {:.4} (n={})",
            diff_sum / diff_n.max(1) as f64,
            diff_n
        );
        let norms: Vec<f64> = embeddings
            .iter()
            .map(|seg| {
                seg.embedding
                    .iter()
                    .map(|&x| (x as f64).powi(2))
                    .sum::<f64>()
                    .sqrt()
            })
            .collect();
        println!(
            "Embedding L2 norm: min={:.4} max={:.4} mean={:.4}",
            norms.iter().cloned().fold(f64::INFINITY, f64::min),
            norms.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
            norms.iter().sum::<f64>() / norms.len().max(1) as f64
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

    // ── Metric 1: dominant-speaker accuracy, under the best label mapping ──
    //
    // Predicted labels are arbitrary cluster IDs (SPEAKER_00, SPEAKER_01,
    // ...) with no inherent correspondence to reference names, so try every
    // possible mapping from predicted labels to reference names and keep
    // whichever maximizes agreement --- the diarization is being scored on
    // whether it separates speakers consistently, not on guessing which
    // cluster ID means which person.
    let mut predicted_labels: Vec<String> = predicted.iter().map(|p| p.speaker.clone()).collect();
    predicted_labels.sort();
    predicted_labels.dedup();

    let mut reference_speakers: Vec<String> = reference.iter().map(|t| t.speaker.clone()).collect();
    reference_speakers.sort();
    reference_speakers.dedup();

    let per_turn_dominant: Vec<(Option<String>, &str)> = reference
        .iter()
        .map(|t| {
            (
                dominant_speaker(t.start, t.end, &predicted).map(str::to_string),
                t.speaker.as_str(),
            )
        })
        .collect();

    let mut best_accuracy = 0.0f64;
    let mut best_mapping: Vec<(String, String)> = Vec::new();

    // Small enough (2-3 speakers) to brute-force every mapping rather than
    // implementing the Hungarian algorithm for what's fundamentally a
    // validation script.
    for perm in permutations(&reference_speakers) {
        if perm.len() < predicted_labels.len() {
            continue;
        }
        let mapping: std::collections::HashMap<&str, &str> = predicted_labels
            .iter()
            .zip(perm.iter())
            .map(|(p, r)| (p.as_str(), r.as_str()))
            .collect();

        let matches = per_turn_dominant
            .iter()
            .filter(|(pred, truth)| {
                pred.as_deref()
                    .and_then(|p| mapping.get(p))
                    .is_some_and(|mapped| mapped == truth)
            })
            .count();
        let accuracy = matches as f64 / per_turn_dominant.len().max(1) as f64;

        if accuracy > best_accuracy {
            best_accuracy = accuracy;
            best_mapping = mapping.into_iter().map(|(p, r)| (p.to_string(), r.to_string())).collect();
        }
    }

    // ── Metric 2: boundary detection ────────────────────────────────────────
    let reference_changes: Vec<f64> = reference
        .windows(2)
        .filter(|w| w[0].speaker != w[1].speaker)
        .map(|w| w[1].start)
        .collect();

    let mut predicted_sorted = predicted.clone();
    predicted_sorted.sort_by(|a, b| a.start.total_cmp(&b.start));
    let predicted_changes: Vec<f64> = predicted_sorted
        .windows(2)
        .filter(|w| w[0].speaker != w[1].speaker)
        .map(|w| w[1].start)
        .collect();

    let detected = reference_changes
        .iter()
        .filter(|&&rc| {
            predicted_changes
                .iter()
                .any(|&pc| (pc - rc).abs() <= BOUNDARY_TOLERANCE_SECS)
        })
        .count();

    if std::env::var("VERBOSE").is_ok() {
        eprintln!("\n=== per-turn debug ===");
        for (turn, (pred, truth)) in reference.iter().zip(per_turn_dominant.iter()) {
            eprintln!(
                "[{:>7.1}-{:>7.1}] truth={:<16} pred={:?}",
                turn.start, turn.end, truth, pred
            );
        }
        eprintln!("\n=== predicted segment timeline (first 40) ===");
        for p in predicted_sorted.iter().take(40) {
            eprintln!("[{:>7.1}-{:>7.1}] {}", p.start, p.end, p.speaker);
        }
    }

    // ── Report ───────────────────────────────────────────────────────────────
    println!("=== Diarization validation: 026_original.mp4 ===");
    println!("Reference turns:        {}", reference.len());
    println!("Reference speakers:     {:?}", reference_speakers);
    println!("Predicted segments:     {}", predicted.len());
    println!("Predicted labels:       {:?}", predicted_labels);
    println!("Best label mapping:     {:?}", best_mapping);
    println!(
        "Dominant-speaker accuracy: {:.1}% ({} / {} turns)",
        best_accuracy * 100.0,
        (best_accuracy * per_turn_dominant.len() as f64).round() as usize,
        per_turn_dominant.len()
    );
    println!(
        "Boundary recall (±{:.0}s): {:.1}% ({} / {} reference speaker changes)",
        BOUNDARY_TOLERANCE_SECS,
        detected as f64 / reference_changes.len().max(1) as f64 * 100.0,
        detected,
        reference_changes.len()
    );

    Ok(())
}

/// All permutations of `items`, as owned `Vec<String>`s. `items.len()` is
/// small (2-3 speakers), so naive recursive generation is plenty fast.
fn permutations(items: &[String]) -> Vec<Vec<String>> {
    if items.is_empty() {
        return vec![Vec::new()];
    }
    let mut result = Vec::new();
    for i in 0..items.len() {
        let mut rest = items.to_vec();
        let item = rest.remove(i);
        for mut tail in permutations(&rest) {
            tail.insert(0, item.clone());
            result.push(tail);
        }
    }
    result
}
