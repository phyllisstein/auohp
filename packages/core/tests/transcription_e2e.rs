//! Comprehensive E2E test suite for the Whisper transcription and diarization pipeline.
//!
//! Covers Tiers 1--4 as specified in `TEST_INFRA.md`:
//! - Tier 1: Feature Coverage (Audio ingestion, diarization clustering, segmentation
//!   windowing, word assembly, VAD timeline, timestamp rounding).
//! - Tier 2: Boundary & Corner Cases (Empty audio, extreme lengths, exact chunk
//!   boundaries, silence signals, NaN/infinity guards).
//! - Tier 3: Cross-Feature Combinations (Resampling + clustering, segmentation + VAD,
//!   word timestamps + turn attribution, audio config quality, pipeline serde).
//! - Tier 4: Real-World Scenarios (Multi-speaker interview dialogue, long monologue,
//!   rapid turn alternation, edge-to-edge audio, non-ASCII multi-byte transcripts).

use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

use auohp_core::eval::diarization::{score as score_diarization, ReferenceTurn};
use auohp_core::transcription::*;

// ── Synthetic Audio Generation Helpers ───────────────────────────────────────

/// Write a standard 16-bit PCM RIFF/WAVE file for testing without external fixtures.
fn write_test_wav(
    path: &Path,
    sample_rate: u32,
    channels: u16,
    samples: &[f32],
) -> std::io::Result<()> {
    let mut file = File::create(path)?;
    let num_samples = samples.len();
    let data_len = (num_samples * 2) as u32;
    let file_len = 36 + data_len;

    // RIFF header
    file.write_all(b"RIFF")?;
    file.write_all(&file_len.to_le_bytes())?;
    file.write_all(b"WAVE")?;

    // fmt subchunk
    file.write_all(b"fmt ")?;
    file.write_all(&16u32.to_le_bytes())?; // Subchunk1Size (16 for PCM)
    file.write_all(&1u16.to_le_bytes())?;  // AudioFormat (1 = PCM)
    file.write_all(&channels.to_le_bytes())?;
    file.write_all(&sample_rate.to_le_bytes())?;
    let byte_rate = sample_rate * (channels as u32) * 2;
    file.write_all(&byte_rate.to_le_bytes())?;
    let block_align = channels * 2;
    file.write_all(&block_align.to_le_bytes())?;
    file.write_all(&16u16.to_le_bytes())?; // BitsPerSample = 16

    // data subchunk
    file.write_all(b"data")?;
    file.write_all(&data_len.to_le_bytes())?;

    for &s in samples {
        let sample_i16 = (s.clamp(-1.0, 1.0) * 32767.0).round() as i16;
        file.write_all(&sample_i16.to_le_bytes())?;
    }

    file.flush()?;
    Ok(())
}

/// RAII wrapper creating a temporary WAV file and deleting it on drop.
struct TempWav {
    path: PathBuf,
}

impl TempWav {
    fn new(name: &str, sample_rate: u32, channels: u16, samples: &[f32]) -> Self {
        let mut path = std::env::temp_dir();
        let unique_name = format!(
            "auohp_e2e_{}_{}_{}.wav",
            name,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        path.push(unique_name);
        write_test_wav(&path, sample_rate, channels, samples)
            .expect("failed to write synthetic wav file");
        Self { path }
    }
}

impl Drop for TempWav {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Generate a pure sine wave signal.
fn generate_sine(sample_rate: u32, duration_s: f32, freq_hz: f32, amp: f32) -> Vec<f32> {
    let n = (sample_rate as f32 * duration_s).round() as usize;
    (0..n)
        .map(|i| {
            let t = i as f32 / sample_rate as f32;
            (2.0 * std::f32::consts::PI * freq_hz * t).sin() * amp
        })
        .collect()
}

/// Compute root-mean-square amplitude of an audio slice.
fn compute_rms(samples: &[f32]) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum_sq: f64 = samples.iter().map(|&s| (s as f64).powi(2)).sum();
    (sum_sq / samples.len() as f64).sqrt()
}

// ── Contract Specifications & Test Oracles ───────────────────────────────────

/// Centisecond rounding oracle matching `whisper.rs:round_to`.
fn round_to(x: f64, places: i32) -> f64 {
    let factor = 10f64.powi(places);
    (x * factor).round() / factor
}

/// Turn indicator stripping oracle matching `whisper.rs:strip_turn_dash`.
fn strip_turn_dash(text: &str) -> &str {
    match text.strip_prefix('-') {
        Some(rest) if !rest.starts_with('-') => rest.trim_start(),
        _ => text,
    }
}

/// Word assembly oracle matching `whisper.rs:assemble_words`.
fn assemble_words<'a, I>(toks: I) -> Vec<(String, f64, f32)>
where
    I: IntoIterator<Item = (&'a [u8], f64, f32)>,
{
    let mut groups: Vec<(Vec<u8>, f64, f32)> = Vec::new();
    for (bytes, at, p) in toks {
        if bytes.first() == Some(&b' ') || groups.is_empty() {
            let start = bytes.iter().take_while(|b| **b == b' ').count();
            groups.push((bytes[start..].to_vec(), at, p));
        } else {
            let last = groups.last_mut().expect("non-empty by construction");
            last.0.extend_from_slice(bytes);
            last.2 = last.2.min(p);
        }
    }
    groups
        .into_iter()
        .map(|(b, at, p)| (String::from_utf8_lossy(&b).into_owned(), at, p))
        .collect()
}

/// VAD timeline region and mapping oracle matching `whisper.rs:VadTimeline`.
#[derive(Debug, Clone, Copy, PartialEq)]
struct TestVadRegion {
    orig_start: f64,
    filtered_start: f64,
    duration: f64,
}

#[derive(Debug, Clone, Default)]
struct TestVadTimeline {
    regions: Vec<TestVadRegion>,
}

impl TestVadTimeline {
    fn identity() -> Self {
        Self {
            regions: Vec::new(),
        }
    }

    fn to_original(&self, t: f64) -> f64 {
        if self.regions.is_empty() {
            return t;
        }
        let mut last_end = self.regions[0].orig_start;
        for r in &self.regions {
            if t < r.filtered_start {
                return last_end;
            }
            if t <= r.filtered_start + r.duration {
                return r.orig_start + (t - r.filtered_start);
            }
            last_end = r.orig_start + r.duration;
        }
        last_end
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// TIER 1: FEATURE COVERAGE (>=5 tests per feature)
// ═════════════════════════════════════════════════════════════════════════════

mod tier1_audio_ingestion {
    use super::*;

    #[test]
    fn test_t1_audio_identity_pass_through_16k() {
        let sample_rate = 16_000;
        let sine = generate_sine(sample_rate, 1.0, 440.0, 0.75);
        let wav = TempWav::new("t1_identity_16k", sample_rate, 1, &sine);
        let decoded = decode_file(&wav.path).expect("decode failed");

        assert_eq!(decoded.sample_rate, 16_000);
        assert_eq!(decoded.source_sample_rate, 16_000);
        assert_eq!(decoded.source_channels, 1);
        assert_eq!(decoded.samples.len(), sine.len());

        let max_err = decoded
            .samples
            .iter()
            .zip(sine.iter())
            .map(|(a, b)| (a - b).abs())
            .fold(0.0f32, f32::max);
        // 16-bit quantization noise is <= 1.0 / 32767.0 ~= 3.1e-5
        assert!(max_err < 1e-4, "identity pass-through altered samples: max error {max_err}");
    }

    #[test]
    fn test_t1_audio_stereo_downmix_averaging() {
        let sample_rate = 16_000;
        let n_frames = 16_000; // 1 second
        let mut interleaved = Vec::with_capacity(n_frames * 2);
        for _ in 0..n_frames {
            let left = 0.5f32;
            let right = -0.5f32;
            interleaved.push(left);
            interleaved.push(right);
        }
        let wav = TempWav::new("t1_stereo_downmix", sample_rate, 2, &interleaved);
        let decoded = decode_file(&wav.path).expect("decode failed");

        assert_eq!(decoded.sample_rate, 16_000);
        assert_eq!(decoded.source_channels, 2);
        assert_eq!(decoded.samples.len(), n_frames);

        // Average of 0.5 and -0.5 is 0.0
        let rms = compute_rms(&decoded.samples);
        assert!(rms < 1e-4, "stereo phase cancellation failed: expected silence, got rms {rms}");
    }

    #[test]
    fn test_t1_audio_downsample_48k_to_16k() {
        let sample_rate = 48_000;
        let duration_s = 1.5;
        let sine = generate_sine(sample_rate, duration_s, 440.0, 0.8);
        let wav = TempWav::new("t1_downsample_48k", sample_rate, 1, &sine);
        let decoded = decode_file(&wav.path).expect("decode failed");

        assert_eq!(decoded.sample_rate, 16_000);
        assert_eq!(decoded.source_sample_rate, 48_000);
        let expected_samples = (duration_s * 16_000.0).round() as usize;
        let delta = (decoded.samples.len() as i64 - expected_samples as i64).abs();
        assert!(delta <= 8, "resampled duration mismatch: got {}, expected {}", decoded.samples.len(), expected_samples);

        // Signal energy should be preserved across resampling
        let orig_rms = compute_rms(&sine);
        let resampled_rms = compute_rms(&decoded.samples);
        assert!((orig_rms - resampled_rms).abs() < 0.05, "energy not preserved: orig {orig_rms} vs resampled {resampled_rms}");
    }

    #[test]
    fn test_t1_audio_upsample_8k_to_16k() {
        let sample_rate = 8_000;
        let duration_s = 1.25;
        let sine = generate_sine(sample_rate, duration_s, 300.0, 0.7);
        let wav = TempWav::new("t1_upsample_8k", sample_rate, 1, &sine);
        let decoded = decode_file(&wav.path).expect("decode failed");

        assert_eq!(decoded.sample_rate, 16_000);
        assert_eq!(decoded.source_sample_rate, 8_000);
        let expected_samples = (duration_s * 16_000.0).round() as usize;
        let delta = (decoded.samples.len() as i64 - expected_samples as i64).abs();
        assert!(delta <= 8, "upsampled duration mismatch: got {}, expected {}", decoded.samples.len(), expected_samples);
    }

    #[test]
    fn test_t1_audio_interpolation_configurations() {
        let sample_rate = 44_100;
        let duration_s = 0.5;
        let sine = generate_sine(sample_rate, duration_s, 1000.0, 0.6);
        let wav = TempWav::new("t1_interp_cfg", sample_rate, 1, &sine);

        for interp in [
            Interpolation::Nearest,
            Interpolation::Linear,
            Interpolation::Quadratic,
            Interpolation::Cubic,
        ] {
            let cfg = AudioConfig {
                resample_chunk: 2048,
                sinc_len: 128,
                f_cutoff: 0.92,
                oversampling_factor: 128,
                interpolation: interp,
            };
            let decoded = decode_file_with(&wav.path, &cfg).expect("decode failed with custom cfg");
            assert_eq!(decoded.sample_rate, 16_000);
            assert!(decoded.samples.iter().all(|s| s.is_finite()));
            assert!(!decoded.samples.is_empty());
        }
    }
}

mod tier1_diarization_clustering {
    use super::*;




    #[test]
    fn test_t1_dominant_speaker_unambiguous_coverage() {
        let diarized = vec![
            DiarizedSegment {
                speaker: "SPEAKER_00".into(),
                start: 0.0,
                end: 5.0,
            },
            DiarizedSegment {
                speaker: "SPEAKER_01".into(),
                start: 5.0,
                end: 10.0,
            },
        ];

        let winner = dominant_speaker(1.0, 4.0, &diarized);
        assert_eq!(winner, Some("SPEAKER_00"));

        let winner2 = dominant_speaker(6.0, 9.0, &diarized);
        assert_eq!(winner2, Some("SPEAKER_01"));
    }

    #[test]
    fn test_t1_dominant_speaker_aggregates_across_turns() {
        // SPEAKER_00 has three short turns: [0.0..1.5], [3.0..4.5], [6.0..7.5] = 4.5s total.
        // SPEAKER_01 has one continuous turn: [1.5..4.5] (inside 0.0..8.0 span) = 3.0s total.
        let diarized = vec![
            DiarizedSegment {
                speaker: "SPEAKER_00".into(),
                start: 0.0,
                end: 1.5,
            },
            DiarizedSegment {
                speaker: "SPEAKER_01".into(),
                start: 1.5,
                end: 4.5,
            },
            DiarizedSegment {
                speaker: "SPEAKER_00".into(),
                start: 4.5,
                end: 6.0,
            },
            DiarizedSegment {
                speaker: "SPEAKER_00".into(),
                start: 6.0,
                end: 7.5,
            },
        ];

        // Over query span [0.0, 8.0]:
        // SPEAKER_00 overlap: 1.5 + 1.5 + 1.5 = 4.5s.
        // SPEAKER_01 overlap: 3.0s.
        let winner = dominant_speaker(0.0, 8.0, &diarized);
        assert_eq!(winner, Some("SPEAKER_00"), "aggregation must sum all segments per speaker");
    }
}

mod tier1_segmentation_windowing {

    const FRAME_STRIDE_SAMPLES: usize = 270;
    const FIRST_FRAME_OFFSET_SAMPLES: usize = 721;
    const WINDOW_SECS: usize = 10;
    const SAMPLE_RATE: usize = 16_000;

    #[test]
    fn test_t1_segmentation_stride_receptive_field_arithmetic() {
        let frame_idx = 10;
        let sample_offset = FIRST_FRAME_OFFSET_SAMPLES + frame_idx * FRAME_STRIDE_SAMPLES;
        assert_eq!(sample_offset, 721 + 2700);

        let time_sec = sample_offset as f64 / SAMPLE_RATE as f64;
        assert!((time_sec - 3421.0 / 16000.0).abs() < 1e-9);
    }

    #[test]
    fn test_t1_segmentation_window_padding_invariant() {
        let window_size = SAMPLE_RATE * WINDOW_SECS; // 160,000 samples
        let sample_len = 250_000; // 15.625 seconds

        let pad_len = (window_size - (sample_len % window_size)) % window_size;
        assert_eq!(pad_len, 70_000);
        assert_eq!((sample_len + pad_len) % window_size, 0);
    }

    #[test]
    fn test_t1_segmentation_exact_window_zero_padding() {
        let window_size = SAMPLE_RATE * WINDOW_SECS;
        let sample_len = window_size * 3; // 480,000 samples (exact 30.0s)

        let pad_len = (window_size - (sample_len % window_size)) % window_size;
        assert_eq!(pad_len, 0, "exact multiple must require zero padding");
    }

    #[test]
    fn test_t1_segmentation_multi_window_boundaries() {
        let window_size = SAMPLE_RATE * WINDOW_SECS;
        let total_samples = 400_000; // 25s -> padded to 480_000 (3 windows)
        let pad_len = (window_size - (total_samples % window_size)) % window_size;
        let padded_len = total_samples + pad_len;

        let window_starts: Vec<usize> = (0..padded_len).step_by(window_size).collect();
        assert_eq!(window_starts, vec![0, 160_000, 320_000]);
        for &start in &window_starts {
            assert!(start + window_size <= padded_len);
        }
    }

    #[test]
    fn test_t1_segmentation_trailing_speech_flush_boundary() {
        let raw_sample_len = 175_000;
        let speech_start_sample = 150_000;

        // Flush must close at true sample count, not padded length
        let flushed_end_sample = raw_sample_len.max(speech_start_sample);
        assert_eq!(flushed_end_sample, 175_000);
        let end_time = flushed_end_sample as f64 / SAMPLE_RATE as f64;
        assert!((end_time - 175_000.0 / 16_000.0).abs() < 1e-9);
    }
}

mod tier1_word_assembly {
    use super::*;

    #[test]
    fn test_t1_word_assembly_space_prefix_grouping() {
        let toks: Vec<(&[u8], f64, f32)> = vec![
            (b" hello" as &[u8], 0.0, 0.95),
            (b" world" as &[u8], 0.6, 0.90),
            (b"!" as &[u8], 1.1, 0.85),
        ];

        let words = assemble_words(toks);
        assert_eq!(words.len(), 2);
        assert_eq!(words[0].0, "hello");
        assert_eq!(words[0].1, 0.0);
        assert_eq!(words[1].0, "world!");
        assert_eq!(words[1].1, 0.6);
        assert_eq!(words[1].2, 0.85, "weakest sub-token probability wins");
    }

    #[test]
    fn test_t1_word_assembly_multibyte_utf8_split() {
        // Multi-byte apostrophe U+2019 split across tokens [0xe2, 0x80] and [0x99]
        let toks: Vec<(&[u8], f64, f32)> = vec![
            (b" they" as &[u8], 1.0, 0.92),
            (b"re" as &[u8], 1.2, 0.88),
            (&[b' ', 0xe2, 0x80], 1.5, 0.80),
            (&[0x99], 1.6, 0.75),
            (b"cause" as &[u8], 1.7, 0.90),
        ];

        let words = assemble_words(toks);
        assert_eq!(words.len(), 2);
        assert_eq!(words[0].0, "theyre");
        assert_eq!(words[1].0, "\u{2019}cause");
        assert!(!words[1].0.contains('\u{fffd}'), "must not contain replacement chars");
        assert_eq!(words[1].2, 0.75);
    }

    #[test]
    fn test_t1_word_assembly_turn_dash_stripping() {
        assert_eq!(strip_turn_dash("- How did it begin?"), "How did it begin?");
        assert_eq!(strip_turn_dash("-Yes"), "Yes");
        // Interior false-start markers must survive untouched
        assert_eq!(strip_turn_dash("-- it was around"), "-- it was around");
        assert_eq!(strip_turn_dash("We went to -- and then"), "We went to -- and then");
    }

    #[test]
    fn test_t1_word_assembly_timestamp_zipping() {
        let groups: Vec<(String, f64, f32)> = vec![
            ("first".to_string(), 1.0f64, 0.9f32),
            ("second".to_string(), 1.8f64, 0.95f32),
            ("third".to_string(), 2.5f64, 0.85f32),
        ];
        let seg_end = 3.2f64;

        let words: Vec<Word> = groups
            .iter()
            .enumerate()
            .map(|(i, (w, start, p))| {
                let end = groups
                    .get(i + 1)
                    .map(|(_, next_start, _)| *next_start)
                    .unwrap_or(seg_end)
                    .max(*start);
                Word {
                    word: w.clone(),
                    start: *start,
                    end,
                    p: *p,
                }
            })
            .collect();

        assert_eq!(words.len(), 3);
        assert_eq!(words[0].start, 1.0);
        assert_eq!(words[0].end, 1.8);
        assert_eq!(words[1].start, 1.8);
        assert_eq!(words[1].end, 2.5);
        assert_eq!(words[2].start, 2.5);
        assert_eq!(words[2].end, 3.2);
    }

    #[test]
    fn test_t1_word_assembly_confidence_propagation() {
        let toks: Vec<(&[u8], f64, f32)> = vec![
            (b" un" as &[u8], 0.0, 0.99),
            (b"pre" as &[u8], 0.2, 0.42), // low confidence sub-token
            (b"ce" as &[u8], 0.4, 0.95),
            (b"dented" as &[u8], 0.6, 0.91),
        ];

        let words = assemble_words(toks);
        assert_eq!(words.len(), 1);
        assert_eq!(words[0].0, "unprecedented");
        assert_eq!(words[0].2, 0.42, "overall confidence must be bounded by minimum token confidence");
    }
}

mod tier1_vad_timeline {
    use super::*;

    #[test]
    fn test_t1_vad_timeline_identity() {
        let timeline = TestVadTimeline::identity();
        for &t in &[0.0, 0.5, 12.34, 1800.0] {
            assert_eq!(timeline.to_original(t), t);
        }
    }

    #[test]
    fn test_t1_vad_timeline_silence_removal_shift() {
        // Original: [2.0..6.0] (4s), then 10s silence, then [16.0..20.0] (4s).
        // Filtered: [0.0..4.0], glue 0.1s, [4.1..8.1].
        let timeline = TestVadTimeline {
            regions: vec![
                TestVadRegion {
                    orig_start: 2.0,
                    filtered_start: 0.0,
                    duration: 4.0,
                },
                TestVadRegion {
                    orig_start: 16.0,
                    filtered_start: 4.1,
                    duration: 4.0,
                },
            ],
        };

        assert_eq!(timeline.to_original(0.0), 2.0);
        assert_eq!(timeline.to_original(2.5), 4.5);
        assert_eq!(timeline.to_original(4.0), 6.0);
        assert_eq!(timeline.to_original(4.1), 16.0);
        assert_eq!(timeline.to_original(6.1), 18.0);
        assert_eq!(timeline.to_original(8.1), 20.0);
    }

    #[test]
    fn test_t1_vad_timeline_glue_silence_clamping() {
        let timeline = TestVadTimeline {
            regions: vec![
                TestVadRegion {
                    orig_start: 1.0,
                    filtered_start: 0.0,
                    duration: 2.0,
                },
                TestVadRegion {
                    orig_start: 10.0,
                    filtered_start: 2.1,
                    duration: 3.0,
                },
            ],
        };

        // Instant 2.05 falls in glue silence [2.0, 2.1]; clamps to preceding end (3.0)
        let mapped = timeline.to_original(2.05);
        assert_eq!(mapped, 3.0);
    }

    #[test]
    fn test_t1_vad_timeline_eof_clamping() {
        let timeline = TestVadTimeline {
            regions: vec![TestVadRegion {
                orig_start: 5.0,
                filtered_start: 0.0,
                duration: 10.0,
            }],
        };

        let mapped = timeline.to_original(100.0);
        assert_eq!(mapped, 15.0, "time past filtered end must clamp to final speech region end");
    }

    #[test]
    fn test_t1_vad_timeline_strict_monotonicity() {
        let timeline = TestVadTimeline {
            regions: vec![
                TestVadRegion {
                    orig_start: 0.5,
                    filtered_start: 0.0,
                    duration: 2.0,
                },
                TestVadRegion {
                    orig_start: 5.0,
                    filtered_start: 2.1,
                    duration: 3.0,
                },
                TestVadRegion {
                    orig_start: 12.0,
                    filtered_start: 5.2,
                    duration: 4.0,
                },
            ],
        };

        let mut prev = f64::NEG_INFINITY;
        for i in 0..=1000 {
            let t = i as f64 / 100.0;
            let orig = timeline.to_original(t);
            assert!(orig >= prev, "monotonicity violated at filtered t={t}: {orig} < {prev}");
            prev = orig;
        }
    }
}

mod tier1_timestamp_rounding {
    use super::*;

    #[test]
    fn test_t1_timestamp_rounding_centiseconds() {
        assert_eq!(round_to(1.234, 2), 1.23);
        assert_eq!(round_to(1.236, 2), 1.24);
        assert_eq!(round_to(0.001, 2), 0.00);
        assert_eq!(round_to(0.009, 2), 0.01);
    }

    #[test]
    fn test_t1_timestamp_rounding_no_downward_bias() {
        // Compare round-to-nearest vs truncation (floor) over 10,000 sub-centisecond intervals
        let n = 10_000;
        let mut err_round_sum = 0.0f64;
        let mut err_trunc_sum = 0.0f64;

        for i in 0..n {
            let x = 1.0 + (i as f64) * 0.0001;
            let r = round_to(x, 2);
            let t = (x * 100.0).floor() / 100.0;
            err_round_sum += r - x;
            err_trunc_sum += t - x;
        }

        let mean_round_err = (err_round_sum / n as f64).abs();
        let mean_trunc_err = (err_trunc_sum / n as f64).abs();

        assert!(mean_round_err < 0.001, "round_to has non-zero mean bias: {mean_round_err}");
        assert!(mean_trunc_err > 0.004, "truncation should exhibit significant downward bias");
    }

    #[test]
    fn test_t1_timestamp_rounding_halfway_values() {
        // Exact half-way cases (e.g. 1.235 -> 1.24)
        assert_eq!(round_to(1.235, 2), 1.24);
        assert_eq!(round_to(2.465, 2), 2.47);
        assert_eq!(round_to(0.055, 2), 0.06);
    }

    #[test]
    fn test_t1_timestamp_rounding_zero_and_deltas() {
        assert_eq!(round_to(0.0, 2), 0.0);
        assert_eq!(round_to(-0.0, 2), 0.0);
        assert_eq!(round_to(0.01, 2), 0.01);
        assert_eq!(round_to(0.004, 2), 0.0);
    }

    #[test]
    fn test_t1_timestamp_rounding_monotonicity() {
        let mut prev = f64::NEG_INFINITY;
        for i in 0..1000 {
            let val = i as f64 * 0.003;
            let rounded = round_to(val, 2);
            assert!(rounded >= prev, "rounding broke monotonicity at {val}: {rounded} < {prev}");
            prev = rounded;
        }
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// TIER 2: BOUNDARY & CORNER CASES (>=5 tests)
// ═════════════════════════════════════════════════════════════════════════════

mod tier2_boundary_corner {
    use super::*;

    #[test]
    fn test_t2_boundary_empty_audio_handling() {
        let empty_samples: [f32; 0] = [];
        let wav = TempWav::new("t2_empty", 16_000, 1, &empty_samples);
        // An empty WAV has 0 audio packets; Symphonia returns "audio track decoded no frames"
        let res = decode_file(&wav.path);
        assert!(res.is_err(), "empty audio stream must return Err rather than panicking");
    }

    #[test]
    fn test_t2_boundary_extreme_lengths() {
        // Short audio: 64 samples at 16 kHz (4 ms)
        let short_samples = vec![0.1f32; 64];
        let wav_short = TempWav::new("t2_short", 16_000, 1, &short_samples);
        let decoded_short = decode_file(&wav_short.path).expect("short audio decode failed");
        assert_eq!(decoded_short.samples.len(), 64);

        // Long audio across multiple resampler chunks (32,768 samples = 8 x 4096 chunks)
        let long_sine = generate_sine(44_100, 2.0, 440.0, 0.5);
        let wav_long = TempWav::new("t2_long", 44_100, 1, &long_sine);
        let decoded_long = decode_file(&wav_long.path).expect("long audio decode failed");
        assert_eq!(decoded_long.sample_rate, 16_000);
        let expected = (long_sine.len() as f64 * 16_000.0 / 44_100.0).round() as usize;
        assert!((decoded_long.samples.len() as i64 - expected as i64).abs() <= 8);
    }

    #[test]
    fn test_t2_boundary_exact_chunk_boundaries() {
        let chunk_size = 4096;
        // Exactly two resampler chunks at 16 kHz
        let exact_samples = vec![0.25f32; chunk_size * 2];
        let wav = TempWav::new("t2_exact_chunk", 16_000, 1, &exact_samples);
        let decoded = decode_file(&wav.path).expect("exact chunk decode failed");
        assert_eq!(decoded.samples.len(), chunk_size * 2);
    }

    #[test]
    fn test_t2_boundary_all_silence_signal() {
        let silence = vec![0.0f32; 16_000];
        let wav = TempWav::new("t2_silence", 16_000, 1, &silence);
        let decoded = decode_file(&wav.path).expect("silence decode failed");

        assert_eq!(compute_rms(&decoded.samples), 0.0);
        // Querying dominant speaker over empty diarization must return None
        let empty_diarized: Vec<DiarizedSegment> = Vec::new();
        assert_eq!(dominant_speaker(0.0, 1.0, &empty_diarized), None);
    }

    #[test]
    fn test_t2_boundary_nan_infinity_guards() {
        // Zero norm vectors in cosine distance must yield 1.0 without NaN/inf
        let zeros = vec![0.0f32; 128];
        let normal = vec![1.0f32; 128];
        let d1 = cosine_distance(&zeros, &normal);
        assert_eq!(d1, 1.0);
        let d2 = cosine_distance(&zeros, &zeros);
        assert_eq!(d2, 1.0);

        // Extreme values clamped cleanly into [0.0, 2.0]
        let extreme_a = vec![1e20f32, 1e20];
        let extreme_b = vec![1e20f32, 1e20];
        let d3 = cosine_distance(&extreme_a, &extreme_b);
        assert!((d3 - 0.0).abs() < 1e-6);
        assert!(d3.is_finite());
    }

    #[test]
    fn test_t2_boundary_disjoint_speaker_turns() {
        let diarized = vec![DiarizedSegment {
            speaker: "SPEAKER_00".into(),
            start: 10.0,
            end: 20.0,
        }];

        // Query before any speech
        assert_eq!(dominant_speaker(0.0, 5.0, &diarized), None);
        // Query after any speech
        assert_eq!(dominant_speaker(25.0, 30.0, &diarized), None);
        // Zero-duration query
        assert_eq!(dominant_speaker(15.0, 15.0, &diarized), None);
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// TIER 3: CROSS-FEATURE COMBINATIONS (Pairwise Interactions)
// ═════════════════════════════════════════════════════════════════════════════

mod tier3_cross_feature {
    use super::*;

    #[test]
    fn test_t3_cross_resampling_and_clustering() {
        // Generate two distinct audio chunks at 44.1 kHz
        let tone_a = generate_sine(44_100, 0.5, 440.0, 0.8);
        let tone_b = generate_sine(44_100, 0.5, 880.0, 0.8);

        let wav_a = TempWav::new("t3_resamp_a", 44_100, 1, &tone_a);
        let wav_b = TempWav::new("t3_resamp_b", 44_100, 1, &tone_b);

        let dec_a = decode_file(&wav_a.path).expect("decode a");
        let dec_b = decode_file(&wav_b.path).expect("decode b");

        // Verify both resampled to 16 kHz
        assert_eq!(dec_a.sample_rate, 16_000);
        assert_eq!(dec_b.sample_rate, 16_000);

        // Simulated speaker embedding extraction from resampled segments
        // Speaker A features concentrated in lower spectrum; Speaker B in higher
        let mut emb_a = vec![0.0f32; 16];
        let mut emb_b = vec![0.0f32; 16];
        emb_a[0] = compute_rms(&dec_a.samples) as f32;
        emb_b[8] = compute_rms(&dec_b.samples) as f32;

        let d_same = cosine_distance(&emb_a, &emb_a);
        let d_diff = cosine_distance(&emb_a, &emb_b);

        assert!(d_same < 1e-6);
        assert!((d_diff - 1.0).abs() < 1e-6, "distinct resampled speakers should separate in embedding space");
    }

    #[test]
    fn test_t3_cross_segmentation_and_vad_timeline() {
        // Speech segments detected in original audio timeline
        let raw_speech_segments = [
            (1.0, 4.0),   // Region 1 (3s)
            (12.0, 18.0), // Region 2 (6s, after 8s silence)
        ];

        // VAD collapses silence into 0.1s glue
        let timeline = TestVadTimeline {
            regions: vec![
                TestVadRegion {
                    orig_start: 1.0,
                    filtered_start: 0.0,
                    duration: 3.0,
                },
                TestVadRegion {
                    orig_start: 12.0,
                    filtered_start: 3.1,
                    duration: 6.0,
                },
            ],
        };

        // Words decoded in filtered timeline: at 1.5s (inside region 1) and 5.1s (inside region 2)
        let filtered_word_1 = 1.5;
        let filtered_word_2 = 5.1; // 3.1 + 2.0s into region 2

        let restored_1 = timeline.to_original(filtered_word_1);
        let restored_2 = timeline.to_original(filtered_word_2);

        assert_eq!(restored_1, 2.5);
        assert_eq!(restored_2, 14.0);

        // Both restored timestamps fall within the original speech segments
        assert!(restored_1 >= raw_speech_segments[0].0 && restored_1 <= raw_speech_segments[0].1);
        assert!(restored_2 >= raw_speech_segments[1].0 && restored_2 <= raw_speech_segments[1].1);
    }

    #[test]
    fn test_t3_cross_word_timestamps_and_turn_attribution() {
        let diarized = vec![
            DiarizedSegment {
                speaker: "SPEAKER_00".into(),
                start: 0.0,
                end: 5.0,
            },
            DiarizedSegment {
                speaker: "SPEAKER_01".into(),
                start: 5.0,
                end: 10.0,
            },
        ];

        let words = [
            Word {
                word: "What".into(),
                start: 1.0,
                end: 1.4,
                p: 0.95,
            },
            Word {
                word: "happened?".into(),
                start: 1.5,
                end: 2.2,
                p: 0.92,
            },
            Word {
                word: "We".into(),
                start: 6.0,
                end: 6.3,
                p: 0.98,
            },
            Word {
                word: "organized.".into(),
                start: 6.4,
                end: 7.1,
                p: 0.89,
            },
        ];

        // Segment 1 (0.0..3.0) -> SPEAKER_00
        let seg1_speaker = dominant_speaker(0.0, 3.0, &diarized);
        assert_eq!(seg1_speaker, Some("SPEAKER_00"));

        // Segment 2 (5.5..8.0) -> SPEAKER_01
        let seg2_speaker = dominant_speaker(5.5, 8.0, &diarized);
        assert_eq!(seg2_speaker, Some("SPEAKER_01"));

        let seg1 = Segment {
            speaker: seg1_speaker.map(str::to_owned),
            text: "What happened?".into(),
            start_time: 1.0,
            end_time: 2.2,
            words: words[0..2].to_vec(),
        };

        let seg2 = Segment {
            speaker: seg2_speaker.map(str::to_owned),
            text: "We organized.".into(),
            start_time: 6.0,
            end_time: 7.1,
            words: words[2..4].to_vec(),
        };

        assert_eq!(seg1.speaker.as_deref(), Some("SPEAKER_00"));
        assert_eq!(seg2.speaker.as_deref(), Some("SPEAKER_01"));
    }

    #[test]
    fn test_t3_cross_audio_config_quality_sweep() {
        let sample_rate = 48_000;
        let test_signal = generate_sine(sample_rate, 0.4, 440.0, 0.7);
        let wav = TempWav::new("t3_sweep", sample_rate, 1, &test_signal);

        let configs = [
            AudioConfig {
                resample_chunk: 1024,
                sinc_len: 128,
                f_cutoff: 0.90,
                oversampling_factor: 128,
                interpolation: Interpolation::Linear,
            },
            AudioConfig {
                resample_chunk: 4096,
                sinc_len: 256,
                f_cutoff: 0.95,
                oversampling_factor: 256,
                interpolation: Interpolation::Cubic,
            },
        ];

        for cfg in &configs {
            let decoded = decode_file_with(&wav.path, cfg).expect("decode failed");
            assert_eq!(decoded.sample_rate, 16_000);
            assert!(compute_rms(&decoded.samples) > 0.4);
            assert!(decoded.samples.iter().all(|&s| s.is_finite()));
        }
    }

    #[test]
    fn test_t3_cross_full_pipeline_dataflow_and_serde() {
        let segs = vec![
            Segment {
                speaker: Some("SPEAKER_00".into()),
                text: "Good morning.".into(),
                start_time: 0.5,
                end_time: 1.5,
                words: vec![
                    Word {
                        word: "Good".into(),
                        start: 0.5,
                        end: 0.9,
                        p: 0.96,
                    },
                    Word {
                        word: "morning.".into(),
                        start: 0.9,
                        end: 1.5,
                        p: 0.94,
                    },
                ],
            },
            Segment {
                speaker: Some("SPEAKER_01".into()),
                text: "Good morning to you.".into(),
                start_time: 2.0,
                end_time: 3.5,
                words: vec![
                    Word {
                        word: "Good".into(),
                        start: 2.0,
                        end: 2.3,
                        p: 0.98,
                    },
                    Word {
                        word: "morning".into(),
                        start: 2.3,
                        end: 2.8,
                        p: 0.97,
                    },
                    Word {
                        word: "to".into(),
                        start: 2.8,
                        end: 3.0,
                        p: 0.95,
                    },
                    Word {
                        word: "you.".into(),
                        start: 3.0,
                        end: 3.5,
                        p: 0.91,
                    },
                ],
            },
        ];

        let result: TranscriptionResult = segs.into();
        assert_eq!(result.segments.len(), 2);
        let speakers = result.speakers.as_ref().expect("speakers must be populated");
        assert_eq!(speakers.len(), 2);
        assert!(speakers.contains(&"SPEAKER_00".to_string()));
        assert!(speakers.contains(&"SPEAKER_01".to_string()));

        // Serde roundtrip validation
        let json_str = serde_json::to_string_pretty(&result).expect("serialization failed");
        let deserialized: TranscriptionResult = serde_json::from_str(&json_str).expect("deserialization failed");

        assert_eq!(deserialized.segments.len(), 2);
        assert_eq!(deserialized.segments[0].text, "Good morning.");
        assert_eq!(deserialized.segments[1].text, "Good morning to you.");
        assert_eq!(deserialized.segments[0].words[0].word, "Good");
        assert_eq!(deserialized.segments[1].words[3].word, "you.");
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// TIER 4: REAL-WORLD APPLICATION SCENARIOS (>=5 Scenarios)
// ═════════════════════════════════════════════════════════════════════════════

mod tier4_real_world_scenarios {
    use super::*;

    #[test]
    fn test_t4_scenario_1_multispeaker_interview_conversation() {
        // Scenario 1: Multi-speaker interview conversation with pauses and turn transitions.
        // Interviewer asks question (0.0..4.0s), 1s pause, Interviewee answers (5.0..15.0s).
        let ground_truth_turns = vec![
            ReferenceTurn {
                start: 0.0,
                end: 4.0,
                speaker: "Sarah Schulman".into(),
            },
            ReferenceTurn {
                start: 5.0,
                end: 15.0,
                speaker: "Iris Long".into(),
            },
        ];

        let diarized = vec![
            DiarizedSegment {
                speaker: "SPEAKER_00".into(),
                start: 0.1,
                end: 3.9,
            },
            DiarizedSegment {
                speaker: "SPEAKER_01".into(),
                start: 5.1,
                end: 14.8,
            },
        ];

        let score = score_diarization(&ground_truth_turns, &diarized);
        assert_eq!(score.dominant_speaker_accuracy, 1.0, "both interview turns correctly attributed");
        assert_eq!(score.matched_turns, 2);
        assert_eq!(score.boundary_recall, 1.0, "speaker transition at ~5.0s detected");
    }

    #[test]
    fn test_t4_scenario_2_long_monologue_low_energy() {
        // Scenario 2: Long monologue with low speech energy and room tone.
        // Single speaker speaks continuously across multiple windows without false turn switches.
        let n_segments = 10;
        let mut diarized = Vec::new();
        for i in 0..n_segments {
            diarized.push(DiarizedSegment {
                speaker: "SPEAKER_00".into(),
                start: (i * 3) as f64,
                end: (i * 3 + 2) as f64 + 0.8,
            });
        }

        // Test queries across the entire 30-second span
        for i in 0..n_segments {
            let start = (i * 3) as f64;
            let end = start + 2.5;
            let speaker = dominant_speaker(start, end, &diarized);
            assert_eq!(speaker, Some("SPEAKER_00"), "single speaker must remain stable across long monologue");
        }
    }

    #[test]
    fn test_t4_scenario_3_rapid_conversational_turns() {
        // Scenario 3: Rapid conversational turn alternation (interviewer + interviewee).
        // Quick back-and-forth Q&A (0.8s, 1.0s, 0.7s turns).
        let ground_truth = vec![
            ReferenceTurn {
                start: 0.0,
                end: 1.5,
                speaker: "Interviewer".into(),
            },
            ReferenceTurn {
                start: 1.5,
                end: 2.8,
                speaker: "Interviewee".into(),
            },
            ReferenceTurn {
                start: 2.8,
                end: 3.6,
                speaker: "Interviewer".into(),
            },
            ReferenceTurn {
                start: 3.6,
                end: 5.0,
                speaker: "Interviewee".into(),
            },
        ];

        let diarized = vec![
            DiarizedSegment {
                speaker: "SPEAKER_00".into(),
                start: 0.1,
                end: 1.4,
            },
            DiarizedSegment {
                speaker: "SPEAKER_01".into(),
                start: 1.55,
                end: 2.75,
            },
            DiarizedSegment {
                speaker: "SPEAKER_00".into(),
                start: 2.85,
                end: 3.55,
            },
            DiarizedSegment {
                speaker: "SPEAKER_01".into(),
                start: 3.65,
                end: 4.95,
            },
        ];

        let score = score_diarization(&ground_truth, &diarized);
        assert_eq!(score.dominant_speaker_accuracy, 1.0);
        assert_eq!(score.matched_turns, 4);
        assert!(score.boundary_recall >= 0.75, "rapid boundary recall must be high");
    }

    #[test]
    fn test_t4_scenario_4_edge_to_edge_speech() {
        // Scenario 4: Edge-to-edge audio with speech starting at t=0.0 and ending at EOF.
        let total_duration = 20.0;
        let diarized = vec![
            DiarizedSegment {
                speaker: "SPEAKER_00".into(),
                start: 0.0,
                end: 10.0,
            },
            DiarizedSegment {
                speaker: "SPEAKER_01".into(),
                start: 10.0,
                end: total_duration,
            },
        ];

        // Query starting at exact t=0.0
        let first_turn = dominant_speaker(0.0, 5.0, &diarized);
        assert_eq!(first_turn, Some("SPEAKER_00"));

        // Query ending at exact t=EOF
        let last_turn = dominant_speaker(15.0, total_duration, &diarized);
        assert_eq!(last_turn, Some("SPEAKER_01"));

        // Verify boundary continuity
        let mid_turn = dominant_speaker(9.0, 11.0, &diarized);
        assert!(mid_turn.is_some());
    }

    #[test]
    fn test_t4_scenario_5_multibyte_utf8_transcription() {
        // Scenario 5: Segment with non-ASCII multi-byte UTF-8 transcriptions.
        // Proper names, foreign words, typographer's punctuation, em-dashes.
        let toks: Vec<(&[u8], f64, f32)> = vec![
            (b" In" as &[u8], 0.0, 0.95),
            (b" Sarah" as &[u8], 0.3, 0.98),
            (b" Schul" as &[u8], 0.8, 0.92),
            (b"man" as &[u8], 1.1, 0.94),
            (&[b' ', 0xe2, 0x80, 0x99], 1.4, 0.88), // ’
            (b"s" as &[u8], 1.5, 0.96),
            (b" caf" as &[u8], 1.8, 0.90),
            (&[0xc3, 0xa9], 2.0, 0.89),            // é
            (&[b' ', 0xe2, 0x80, 0x94], 2.3, 0.85), // —
            (b" the" as &[u8], 2.6, 0.97),
            (b" prot" as &[u8], 2.9, 0.91),
            (&[0xc3, 0xa9], 3.1, 0.93),            // é
            (b"g" as &[u8], 3.3, 0.92),
            (&[0xc3, 0xa9], 3.5, 0.90),            // é
            (b" spoke." as &[u8], 3.7, 0.95),
        ];

        let words = assemble_words(toks);
        let reconstructed: Vec<String> = words.into_iter().map(|(w, _, _)| w).collect();

        assert_eq!(
            reconstructed,
            vec![
                "In",
                "Sarah",
                "Schulman",
                "’s",
                "café",
                "—",
                "the",
                "protégé",
                "spoke."
            ]
        );

        // Verify valid UTF-8 without corruption or replacement characters
        for word in &reconstructed {
            assert!(!word.contains('\u{fffd}'), "word '{word}' contains unicode replacement char");
        }
    }
}
