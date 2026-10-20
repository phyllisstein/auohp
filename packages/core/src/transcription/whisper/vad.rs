//! Voice Activity Detection (VAD) via Silero VAD.
//!
//! `FullParams::enable_vad` is ignored by `whisper_full_with_state` (whisper.cpp
//! reads `params.vad` only in `whisper_full` and `whisper_full_parallel`), so
//! [`apply_vad`] runs Silero explicitly before decoding. Speech regions are glued
//! with [`VAD_GLUE_SECONDS`] of silence, so Whisper's timestamps refer to the
//! filtered timeline; [`VadTimeline`] maps them back to the original recording.

use std::path::Path;

use anyhow::{Context as _, Result};
use whisper_rs::{WhisperVadContext, WhisperVadContextParams, WhisperVadParams};

use super::super::config::VadConfig;

/// Audio sample rate (16,000 Hz) used for sample count to time conversions in VAD.
const SAMPLE_RATE: f64 = 16_000.0;

/// Silence inserted between kept speech regions, as whisper.cpp does.
pub const VAD_GLUE_SECONDS: f64 = 0.1;

/// Filename of the silero VAD ggml model under `$MODELS_DIR`, as
/// `scripts/download-models.py` writes it.
pub const VAD_MODEL_FILE: &str = "ggml-silero-v6.2.0.bin";

/// One kept speech region, in both timelines.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VadRegion {
    /// Where the region starts in the real recording.
    pub orig_start: f64,
    /// Where it starts in the filtered audio Whisper actually sees.
    pub filtered_start: f64,
    pub duration: f64,
}

/// Maps filtered-audio time back to real-recording time. Every time Whisper
/// reports must pass through [`VadTimeline::to_original`].
#[derive(Debug, Clone, Default)]
pub struct VadTimeline {
    pub(crate) regions: Vec<VadRegion>,
}

impl VadTimeline {
    /// Identity map, for when VAD is disabled and the two timelines are the same.
    pub fn identity() -> Self {
        Self {
            regions: Vec::new(),
        }
    }

    /// Returns true if this timeline does not alter timestamps.
    pub fn is_identity(&self) -> bool {
        self.regions.is_empty()
    }

    /// Map an instant in the filtered timeline back to the real recording.
    ///
    /// Times in the glue silence between regions clamp to the end of the
    /// preceding region.
    pub fn to_original(&self, t: f64) -> f64 {
        if self.is_identity() {
            return t;
        }
        let mut last_end = self.regions[0].orig_start;
        for r in &self.regions {
            if t < r.filtered_start {
                return last_end; // inside glue silence
            }
            if t <= r.filtered_start + r.duration {
                return r.orig_start + (t - r.filtered_start);
            }
            last_end = r.orig_start + r.duration;
        }
        last_end
    }
}

/// Run silero over `samples` and return the filtered audio plus its timeline map.
///
/// Mirrors whisper.cpp's construction: each region is kept, all but the last
/// are extended by `samples_overlap`, and regions are glued with 0.1 s of silence.
pub fn apply_vad(
    samples: &[f32],
    vad_model_path: &Path,
    cfg: &VadConfig,
) -> Result<(Vec<f32>, VadTimeline)> {
    let path = vad_model_path
        .to_str()
        .context("VAD model path is not valid UTF-8")?;

    let mut vctx = WhisperVadContext::new(path, WhisperVadContextParams::new())
        .map_err(|e| anyhow::anyhow!("failed to load VAD model: {e}"))?;

    let mut params = WhisperVadParams::new();
    if let Some(x) = cfg.threshold {
        params.set_threshold(x);
    }
    if let Some(x) = cfg.min_speech_duration_ms {
        params.set_min_speech_duration(x);
    }
    if let Some(x) = cfg.min_silence_duration_ms {
        params.set_min_silence_duration(x);
    }
    if let Some(x) = cfg.max_speech_duration_s {
        params.set_max_speech_duration(x);
    }
    if let Some(x) = cfg.speech_pad_ms {
        params.set_speech_pad(x);
    }
    if let Some(x) = cfg.samples_overlap_s {
        params.set_samples_overlap(x);
    }

    let segments = vctx
        .segments_from_samples(params, samples)
        .map_err(|e| anyhow::anyhow!("VAD failed: {e}"))?;

    let n = segments.num_segments();
    if n == 0 {
        // No speech: pass the audio through unfiltered rather than emit nothing.
        tracing::warn!("Whisper: VAD found no speech; passing audio through unfiltered");
        return Ok((samples.to_vec(), VadTimeline::identity()));
    }

    let overlap = cfg.samples_overlap_s.unwrap_or(0.1) as f64;
    let total = samples.len() as f64 / SAMPLE_RATE;

    let mut filtered: Vec<f32> = Vec::with_capacity(samples.len());
    let mut regions = Vec::with_capacity(n as usize);

    for i in 0..n {
        let (Some(start_cs), Some(end_cs)) = (
            segments.get_segment_start_timestamp(i),
            segments.get_segment_end_timestamp(i),
        ) else {
            continue;
        };
        // whisper-rs reports centiseconds.
        let orig_start = (start_cs as f64 / 100.0).clamp(0.0, total);
        let mut orig_end = end_cs as f64 / 100.0;
        if i < n - 1 {
            orig_end += overlap;
        }
        let orig_end = orig_end.clamp(orig_start, total);

        let (s, e) = (
            (orig_start * SAMPLE_RATE) as usize,
            (orig_end * SAMPLE_RATE) as usize,
        );
        let (s, e) = (s.min(samples.len()), e.min(samples.len()));
        if e <= s {
            continue;
        }

        if !regions.is_empty() {
            filtered.extend(std::iter::repeat_n(
                0.0,
                (VAD_GLUE_SECONDS * SAMPLE_RATE) as usize,
            ));
        }
        regions.push(VadRegion {
            orig_start,
            filtered_start: filtered.len() as f64 / SAMPLE_RATE,
            duration: (e - s) as f64 / SAMPLE_RATE,
        });
        filtered.extend_from_slice(&samples[s..e]);
    }

    if regions.is_empty() {
        return Ok((samples.to_vec(), VadTimeline::identity()));
    }

    let kept = filtered.len() as f64 / SAMPLE_RATE;
    tracing::debug!(
        "Whisper: VAD kept {} speech regions, {:.1}s of {:.1}s ({:.0}% dropped)",
        regions.len(),
        kept,
        total,
        100.0 * (1.0 - kept / total)
    );

    Ok((filtered, VadTimeline { regions }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two speech regions, 10 s of silence dropped between them.
    ///
    ///   real:     [2.0 .. 5.0]            [15.0 .. 18.0]
    ///   filtered: [0.0 .. 3.0]  glue 0.1  [3.1  .. 6.1]
    fn timeline() -> VadTimeline {
        VadTimeline {
            regions: vec![
                VadRegion {
                    orig_start: 2.0,
                    filtered_start: 0.0,
                    duration: 3.0,
                },
                VadRegion {
                    orig_start: 15.0,
                    filtered_start: 3.1,
                    duration: 3.0,
                },
            ],
        }
    }

    #[test]
    fn default_timeline_is_identity() {
        let t = VadTimeline::default();
        assert!(t.is_identity());
        assert_eq!(t.to_original(42.0), 42.0);
    }

    #[test]
    fn identity_timeline_is_a_no_op() {
        let t = VadTimeline::identity();
        for x in [0.0, 1.5, 900.0] {
            assert_eq!(t.to_original(x), x);
        }
    }

    #[test]
    fn maps_filtered_time_back_to_the_recording() {
        let t = timeline();
        assert_eq!(t.to_original(0.0), 2.0, "first region start");
        assert_eq!(t.to_original(1.5), 3.5, "inside first region");
        assert_eq!(t.to_original(3.0), 5.0, "first region end");
        assert_eq!(t.to_original(3.1), 15.0, "second region start");
        assert!(
            (t.to_original(4.5) - 16.4).abs() < 1e-9,
            "inside second region"
        );
        assert!(
            (t.to_original(6.1) - 18.0).abs() < 1e-9,
            "second region end"
        );
    }

    #[test]
    fn the_gap_the_mapping_exists_to_close() {
        // Unmapped, a word 4.5 s into the filtered audio would be reported at
        // 4.5 s; it is really at 16.4 s, and the error grows with every dropped
        // silence.
        let t = timeline();
        assert!(
            (t.to_original(4.5) - 4.5 - 11.9).abs() < 1e-9,
            "error the map removes"
        );
    }

    #[test]
    fn glue_silence_clamps_to_the_preceding_region() {
        // No audio exists in the glue; clamp to the preceding region.
        assert_eq!(timeline().to_original(3.05), 5.0);
    }

    #[test]
    fn times_past_the_end_clamp_rather_than_extrapolate() {
        // Clamp rather than extrapolate: Whisper can place a trailing timestamp
        // past the last sample.
        assert_eq!(timeline().to_original(99.0), 18.0);
    }

    #[test]
    fn mapping_is_monotonic() {
        // Word order must survive the map.
        let t = timeline();
        let mut prev = f64::NEG_INFINITY;
        for i in 0..=610 {
            let cur = t.to_original(i as f64 / 100.0);
            assert!(cur >= prev, "went backwards at {}: {} < {}", i, cur, prev);
            prev = cur;
        }
    }
}
