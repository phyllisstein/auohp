//! Speaker-diarization turn-taking scoring.
//!
//! Scores who was speaking against a hand-labeled, time-coded reference
//! (`HH:MM:SS:FF`, assumed 30 fps). Separate from [`super::score`] because the
//! reference is time-based, not text-anchored. Not DER; two turn-taking
//! questions per reference turn:
//!
//!   1. **Dominant-speaker accuracy**: does the predicted label holding most of
//!      the turn's span (via [`crate::transcription::dominant_speaker`], the
//!      pipeline's own labeller) match the reference speaker, under the label
//!      mapping that maximizes agreement?
//!   2. **Boundary recall**: is each reference speaker change matched by a
//!      predicted change within a tolerance window?

use std::collections::HashMap;

use anyhow::{Context, Result};

use crate::transcription::{cosine_distance, dominant_speaker, DiarizedSegment, SegmentEmbedding};

const REFERENCE_FPS: f64 = 30.0;

/// How close a predicted speaker change must land to a ground-truth change to
/// count as detected. Generous because hand-cut points never match clustering
/// boundaries frame for frame.
pub const BOUNDARY_TOLERANCE_SECS: f64 = 3.0;

/// One ground-truth speaker turn, parsed from a hand-labeled reference.
#[derive(Debug, Clone)]
pub struct ReferenceTurn {
    pub start: f64,
    pub end: f64,
    pub speaker: String,
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

/// Parse a reference transcript of blocks shaped like:
///
/// ```text
/// 00:00:00:06 - 00:00:25:02
/// Speaker 1
/// <transcript text, ignored --- only the timecode and speaker matter>
/// ```
pub fn parse_reference(text: &str) -> Result<Vec<ReferenceTurn>> {
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

        // Skip the transcript text up to the next blank line.
        for text_line in lines.by_ref() {
            if text_line.trim().is_empty() {
                break;
            }
        }

        turns.push(ReferenceTurn { start, end, speaker });
    }

    Ok(turns)
}

/// Result of scoring predicted diarization against a reference.
#[derive(Debug, Clone)]
pub struct DiarizationScore {
    pub reference_speakers: Vec<String>,
    pub predicted_labels: Vec<String>,
    /// The predicted-label -> reference-name assignment that maximized
    /// dominant-speaker agreement.
    pub best_mapping: Vec<(String, String)>,
    pub dominant_speaker_accuracy: f64,
    pub matched_turns: usize,
    pub total_turns: usize,
    pub boundary_recall: f64,
    pub detected_changes: usize,
    pub total_changes: usize,
    pub boundary_tolerance_secs: f64,
}

/// Score `predicted` diarization against `reference`, using
/// [`BOUNDARY_TOLERANCE_SECS`] for boundary matching.
pub fn score(reference: &[ReferenceTurn], predicted: &[DiarizedSegment]) -> DiarizationScore {
    score_with_tolerance(reference, predicted, BOUNDARY_TOLERANCE_SECS)
}

fn score_with_tolerance(
    reference: &[ReferenceTurn],
    predicted: &[DiarizedSegment],
    boundary_tolerance_secs: f64,
) -> DiarizationScore {
    // Predicted labels are arbitrary cluster IDs, so score under the best
    // label-to-name mapping: this measures consistent separation, not naming.
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
                dominant_speaker(t.start, t.end, predicted).map(str::to_string),
                t.speaker.as_str(),
            )
        })
        .collect();

    let mut best_accuracy = 0.0f64;
    let mut best_mapping: Vec<(String, String)> = Vec::new();

    // 2-3 speakers: brute force beats the Hungarian algorithm.
    for perm in permutations(&reference_speakers) {
        if perm.len() < predicted_labels.len() {
            continue;
        }
        let mapping: HashMap<&str, &str> = predicted_labels
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
    let matched_turns = (best_accuracy * per_turn_dominant.len() as f64).round() as usize;

    let reference_changes: Vec<f64> = reference
        .windows(2)
        .filter(|w| w[0].speaker != w[1].speaker)
        .map(|w| w[1].start)
        .collect();

    let mut predicted_sorted = predicted.to_vec();
    predicted_sorted.sort_by(|a, b| a.start.total_cmp(&b.start));
    let predicted_changes: Vec<f64> = predicted_sorted
        .windows(2)
        .filter(|w| w[0].speaker != w[1].speaker)
        .map(|w| w[1].start)
        .collect();

    let detected_changes = reference_changes
        .iter()
        .filter(|&&rc| {
            predicted_changes
                .iter()
                .any(|&pc| (pc - rc).abs() <= boundary_tolerance_secs)
        })
        .count();
    let total_changes = reference_changes.len();

    DiarizationScore {
        reference_speakers,
        predicted_labels,
        best_mapping,
        dominant_speaker_accuracy: best_accuracy,
        matched_turns,
        total_turns: per_turn_dominant.len(),
        boundary_recall: detected_changes as f64 / total_changes.max(1) as f64,
        detected_changes,
        total_changes,
        boundary_tolerance_secs,
    }
}

/// All permutations of `items`; naive recursion, since there are 2-3 speakers.
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

/// Same-speaker vs. different-speaker embedding distance, bypassing clustering.
/// If the two means are indistinguishable, the embeddings, not the clustering
/// cutoff, are the problem.
#[derive(Debug, Clone)]
pub struct EmbeddingDiagnostic {
    pub segments_embedded: usize,
    pub same_speaker_mean_distance: f64,
    pub same_speaker_pairs: u64,
    pub diff_speaker_mean_distance: f64,
    pub diff_speaker_pairs: u64,
    pub norm_min: f64,
    pub norm_max: f64,
    pub norm_mean: f64,
}

/// Label each embedded segment by whichever reference turn covers most of
/// its span, then compare mean cosine distance for same-speaker pairs
/// against different-speaker pairs.
pub fn embedding_diagnostic(
    embeddings: &[SegmentEmbedding],
    reference: &[ReferenceTurn],
) -> EmbeddingDiagnostic {
    let labeled: Vec<(&str, &[f32])> = embeddings
        .iter()
        .filter_map(|seg| {
            let mid_turn = reference.iter().max_by(|a, b| {
                let overlap =
                    |t: &ReferenceTurn| (seg.end.min(t.end) - seg.start.max(t.start)).max(0.0);
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

    let norms: Vec<f64> = embeddings
        .iter()
        .map(|seg| seg.embedding.iter().map(|&x| (x as f64).powi(2)).sum::<f64>().sqrt())
        .collect();

    EmbeddingDiagnostic {
        segments_embedded: embeddings.len(),
        same_speaker_mean_distance: same_sum / same_n.max(1) as f64,
        same_speaker_pairs: same_n,
        diff_speaker_mean_distance: diff_sum / diff_n.max(1) as f64,
        diff_speaker_pairs: diff_n,
        norm_min: norms.iter().cloned().fold(f64::INFINITY, f64::min),
        norm_max: norms.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
        norm_mean: norms.iter().sum::<f64>() / norms.len().max(1) as f64,
    }
}
