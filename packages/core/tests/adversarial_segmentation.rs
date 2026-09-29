use std::collections::HashSet;
use std::path::Path;

#[path = "../src/transcription/diarize/segmentation.rs"]
mod segmentation;

use auohp_core::transcription::{Segment, TranscriptionResult, Word};
use segmentation::Segmenter;

const MODEL_PATH: &str =
    "/Users/daniel/Code/Personal/auohp/packages/core/models/pyannote-segmentation-3.0.onnx";

fn get_segmenter() -> Option<Segmenter> {
    let path = Path::new(MODEL_PATH);
    if path.exists() {
        Segmenter::new(path).ok()
    } else {
        None
    }
}

// =========================================================================
// 1. Audio Slicing & Zero-Padding Math Properties
// =========================================================================

#[test]
fn test_zero_padding_formula_invariants() {
    let sample_rate = 16000usize;
    let window_secs = 10usize;
    let window_size = sample_rate * window_secs; // 160,000

    // Invariant 1: Empty audio slice produces pad_len = 0
    let pad_len_empty = (window_size - (0 % window_size)) % window_size;
    assert_eq!(pad_len_empty, 0, "Empty slice must have pad_len == 0");

    // Invariant 2: Exact multiples of window_size produce pad_len = 0
    for n in 1..=10 {
        let len = n * window_size;
        let pad_len = (window_size - (len % window_size)) % window_size;
        assert_eq!(
            pad_len, 0,
            "Exact multiple {n}*window_size must have pad_len == 0"
        );
        let padded_len = len + pad_len;
        let num_windows = (0..padded_len).step_by(window_size).count();
        assert_eq!(
            num_windows, n,
            "Exact multiple {n}*window_size must result in exactly {n} windows, never {n}+1"
        );
    }

    // Invariant 3: Slices shorter than 1 window size (0 < len < window_size)
    for len in [1, 2, 10, 270, 721, 1000, 80_000, 159_999] {
        let pad_len = (window_size - (len % window_size)) % window_size;
        assert_eq!(
            len + pad_len,
            window_size,
            "Sub-window len {len} must pad to exactly window_size"
        );
        let num_windows = (0..(len + pad_len)).step_by(window_size).count();
        assert_eq!(
            num_windows, 1,
            "Sub-window len {len} must produce exactly 1 window"
        );
    }

    // Invariant 4: Slices of length N*window_size + 1 (just over multiple)
    for n in 1..=5 {
        let len = n * window_size + 1;
        let pad_len = (window_size - (len % window_size)) % window_size;
        assert_eq!(
            pad_len,
            window_size - 1,
            "len {len} must require window_size - 1 padding samples"
        );
        let padded_len = len + pad_len;
        assert_eq!(padded_len, (n + 1) * window_size);
        let num_windows = (0..padded_len).step_by(window_size).count();
        assert_eq!(
            num_windows,
            n + 1,
            "len {len} must produce exactly {n}+1 windows"
        );
    }
}

// =========================================================================
// 2. Empty Audio Slice (&[]) Stress Test
// =========================================================================

#[test]
fn test_segment_empty_slice() {
    let mut segmenter = match get_segmenter() {
        Some(s) => s,
        None => {
            eprintln!("Skipping model test: model file not found");
            return;
        }
    };

    // Empty slice with valid sample rates
    for &sample_rate in &[8000, 16000, 22050, 44100, 48000] {
        let result = segmenter.segment(&[], sample_rate);
        assert!(
            result.is_ok(),
            "Empty audio slice must succeed for sample_rate={sample_rate}"
        );
        let segments = result.unwrap();
        assert!(
            segments.is_empty(),
            "Empty audio slice must produce zero segments, got {:?}",
            segments
        );
    }

    // Zero sample rate must error out cleanly without panic
    let err_result = segmenter.segment(&[], 0);
    assert!(err_result.is_err(), "sample_rate = 0 must return Err");
    assert!(
        err_result
            .unwrap_err()
            .to_string()
            .contains("sample_rate must be > 0"),
        "Error message should mention sample_rate"
    );
}

// =========================================================================
// 3. Slices Shorter Than 1 Window Size
// =========================================================================

#[test]
fn test_segment_shorter_than_one_window() {
    let mut segmenter = match get_segmenter() {
        Some(s) => s,
        None => return,
    };

    let sample_rate = 16000;
    // Test various sub-window lengths
    let sub_lengths = [
        1,      // 1 sample
        2,      // 2 samples
        16,     // 1 ms
        270,    // 1 frame stride
        721,    // offset samples
        1000,   // ~62.5 ms
        8000,   // 0.5 s
        80_000, // 5.0 s (half window)
        159_999 // 9.9999 s (window_size - 1)
    ];

    for len in sub_lengths {
        // Test with silence
        let silence = vec![0.0f32; len];
        let res = segmenter.segment(&silence, sample_rate);
        assert!(
            res.is_ok(),
            "Silence of len {len} must succeed without panic: {:?}",
            res.err()
        );
        let segments = res.unwrap();
        for seg in &segments {
            assert!(
                seg.start <= seg.end,
                "Segment start ({}) must be <= end ({}) for len {}",
                seg.start,
                seg.end,
                len
            );
            assert!(
                seg.end <= (len as f64 / sample_rate as f64) + 0.05,
                "Segment end ({}) must not extrapolate far beyond audio length ({}s)",
                seg.end,
                len as f64 / sample_rate as f64
            );
        }

        // Test with synthetic tone (440Hz sine wave)
        let tone: Vec<f32> = (0..len)
            .map(|i| (2.0 * std::f32::consts::PI * 440.0 * (i as f32) / (sample_rate as f32)).sin() * 0.5)
            .collect();
        let res_tone = segmenter.segment(&tone, sample_rate);
        assert!(
            res_tone.is_ok(),
            "Tone of len {len} must succeed without panic: {:?}",
            res_tone.err()
        );
        let segs_tone = res_tone.unwrap();
        for seg in &segs_tone {
            assert!(seg.start <= seg.end);
            assert!(seg.start >= 0.0);
        }
    }
}

// =========================================================================
// 4. Slices Exactly Equal to N Window Sizes
// =========================================================================

#[test]
fn test_segment_exact_n_window_sizes() {
    let mut segmenter = match get_segmenter() {
        Some(s) => s,
        None => return,
    };

    let sample_rate = 16000;
    let window_size = sample_rate as usize * 10; // 160,000

    // Test exact 1x window (160,000 samples)
    let samples_1w = vec![0.0f32; window_size];
    let res_1w = segmenter.segment(&samples_1w, sample_rate);
    assert!(res_1w.is_ok(), "Exact 1 window must succeed");
    let segs_1w = res_1w.unwrap();
    for seg in &segs_1w {
        assert!(seg.end <= 10.0 + 0.05);
    }

    // Test exact 2x window (320,000 samples)
    let samples_2w = vec![0.0f32; 2 * window_size];
    let res_2w = segmenter.segment(&samples_2w, sample_rate);
    assert!(res_2w.is_ok(), "Exact 2 windows must succeed");
    let segs_2w = res_2w.unwrap();
    for seg in &segs_2w {
        assert!(seg.end <= 20.0 + 0.05);
    }

    // Test exact 3x window (480,000 samples) with alternating tone and silence
    let mut samples_3w = vec![0.0f32; 3 * window_size];
    // Add a loud 440Hz tone in the middle window (10s to 20s)
    for i in window_size..(2 * window_size) {
        samples_3w[i] = (2.0 * std::f32::consts::PI * 440.0 * (i as f32) / (sample_rate as f32)).sin() * 0.8;
    }
    let res_3w = segmenter.segment(&samples_3w, sample_rate);
    assert!(res_3w.is_ok(), "Exact 3 windows must succeed");
    let segs_3w = res_3w.unwrap();
    for seg in &segs_3w {
        assert!(seg.start >= 0.0);
        assert!(seg.end <= 30.0 + 0.05);
        assert!(seg.start <= seg.end);
    }

    // Test boundary: 1 sample beyond exact window (window_size + 1)
    let samples_1w_plus_1 = vec![0.0f32; window_size + 1];
    let res_plus = segmenter.segment(&samples_1w_plus_1, sample_rate);
    assert!(res_plus.is_ok(), "window_size + 1 must succeed");
}

// =========================================================================
// 5. Slices with f32::NAN and f32::INFINITY Guards / Robustness
// =========================================================================

#[test]
fn test_segment_nan_and_infinity_resilience() {
    let mut segmenter = match get_segmenter() {
        Some(s) => s,
        None => return,
    };

    let sample_rate = 16000;
    let window_size = sample_rate as usize * 10;

    // Test A: All NaNs in full window
    let nan_window = vec![f32::NAN; window_size];
    let res_nan = segmenter.segment(&nan_window, sample_rate);
    assert!(
        res_nan.is_ok(),
        "Segmenter must handle all-NaN input without crashing/panicking: {:?}",
        res_nan.err()
    );

    // Test B: Sub-window with NaNs
    let nan_sub = vec![f32::NAN; 1000];
    let res_nan_sub = segmenter.segment(&nan_sub, sample_rate);
    assert!(
        res_nan_sub.is_ok(),
        "Segmenter must handle sub-window NaN input without crashing: {:?}",
        res_nan_sub.err()
    );

    // Test C: All positive INFINITY
    let inf_window = vec![f32::INFINITY; window_size];
    let res_inf = segmenter.segment(&inf_window, sample_rate);
    assert!(
        res_inf.is_ok(),
        "Segmenter must handle positive INFINITY input without crashing: {:?}",
        res_inf.err()
    );

    // Test D: All negative INFINITY
    let neg_inf_window = vec![f32::NEG_INFINITY; window_size];
    let res_neg_inf = segmenter.segment(&neg_inf_window, sample_rate);
    assert!(
        res_neg_inf.is_ok(),
        "Segmenter must handle negative INFINITY input without crashing: {:?}",
        res_neg_inf.err()
    );

    // Test E: Mixed adversarial floats (NaN, +Inf, -Inf, subnormals, extreme magnitudes)
    let mut mixed = vec![0.0f32; window_size];
    for (i, val) in mixed.iter_mut().enumerate() {
        match i % 7 {
            0 => *val = f32::NAN,
            1 => *val = f32::INFINITY,
            2 => *val = f32::NEG_INFINITY,
            3 => *val = f32::MIN_POSITIVE, // subnormal/denormal
            4 => *val = 1e30_f32,          // huge positive float
            5 => *val = -1e30_f32,         // huge negative float
            _ => *val = 0.5,
        }
    }
    let res_mixed = segmenter.segment(&mixed, sample_rate);
    assert!(
        res_mixed.is_ok(),
        "Segmenter must survive mixed adversarial floats without crashing: {:?}",
        res_mixed.err()
    );
}

// =========================================================================
// 6. TranscriptionResult::from_iter Ordering & Metadata Preservation
// =========================================================================

#[test]
fn test_transcription_result_from_iter_empty() {
    let empty_segments: Vec<Segment> = vec![];
    let result: TranscriptionResult = empty_segments.into_iter().collect();

    assert!(result.segments.is_empty(), "Segments must be empty");
    let speakers = result.speakers.expect("speakers vector must be present");
    assert!(speakers.is_empty(), "Speakers must be empty for empty input");
    assert!(result.models.is_some(), "Models config must be present");
}

#[test]
fn test_transcription_result_from_iter_preserves_exact_order_and_metadata() {
    let test_words = vec![
        Word {
            word: "First".into(),
            start: 0.1,
            end: 0.5,
            p: 0.98,
        },
        Word {
            word: "word".into(),
            start: 0.5,
            end: 0.9,
            p: 0.95,
        },
    ];

    let segments = vec![
        Segment {
            speaker: Some("SPEAKER_B".into()),
            text: "Segment zero, speaker B".into(),
            start_time: 0.0,
            end_time: 1.0,
            words: test_words.clone(),
        },
        Segment {
            speaker: Some("SPEAKER_A".into()),
            text: "Segment one, speaker A".into(),
            start_time: 1.0,
            end_time: 2.5,
            words: vec![],
        },
        Segment {
            speaker: None,
            text: "Segment two, unassigned speaker".into(),
            start_time: 2.5,
            end_time: 3.2,
            words: vec![],
        },
        Segment {
            speaker: Some("SPEAKER_B".into()),
            text: "Segment three, speaker B again".into(),
            start_time: 3.2,
            end_time: 4.8,
            words: test_words.clone(),
        },
        Segment {
            speaker: Some("SPEAKER_C".into()),
            text: "Segment four, speaker C".into(),
            start_time: 4.8,
            end_time: 6.0,
            words: vec![],
        },
    ];

    // Collect via FromIterator
    let result: TranscriptionResult = segments.clone().into_iter().collect();

    // 1. Strict segment count & ordering
    assert_eq!(
        result.segments.len(),
        segments.len(),
        "Segment count must match"
    );
    for (i, (actual, expected)) in result.segments.iter().zip(segments.iter()).enumerate() {
        assert_eq!(
            actual.speaker, expected.speaker,
            "Segment {i} speaker mismatch"
        );
        assert_eq!(actual.text, expected.text, "Segment {i} text mismatch");
        assert_eq!(
            actual.start_time, expected.start_time,
            "Segment {i} start_time mismatch"
        );
        assert_eq!(
            actual.end_time, expected.end_time,
            "Segment {i} end_time mismatch"
        );
        assert_eq!(
            actual.words.len(),
            expected.words.len(),
            "Segment {i} words length mismatch"
        );
        for (w_idx, (actual_w, expected_w)) in
            actual.words.iter().zip(expected.words.iter()).enumerate()
        {
            assert_eq!(
                actual_w.word, expected_w.word,
                "Segment {i} word {w_idx} mismatch"
            );
            assert_eq!(
                actual_w.start, expected_w.start,
                "Segment {i} word {w_idx} start mismatch"
            );
            assert_eq!(
                actual_w.end, expected_w.end,
                "Segment {i} word {w_idx} end mismatch"
            );
            assert_eq!(
                actual_w.p, expected_w.p,
                "Segment {i} word {w_idx} prob mismatch"
            );
        }
    }

    // 2. Speakers metadata: unique speaker set extraction
    let speakers = result.speakers.expect("speakers must be populated");
    let speaker_set: HashSet<String> = speakers.into_iter().collect();
    let expected_speaker_set: HashSet<String> = ["SPEAKER_A", "SPEAKER_B", "SPEAKER_C"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    assert_eq!(
        speaker_set, expected_speaker_set,
        "Unique speaker set must match exactly"
    );

    // 3. Models config metadata
    let models = result.models.expect("models must be populated");
    assert_eq!(models.segmentation_model, "pyannote-segmentation-3.0.onnx");
    assert_eq!(models.vad_model, "ggml-silero-v6.2.0.bin");
    assert_eq!(models.whisper_model, "ggml-large-v3.bin");
}

#[test]
fn test_transcription_result_from_iter_hundred_segments() {
    let count = 100;
    let segments: Vec<Segment> = (0..count)
        .map(|i| Segment {
            speaker: Some(format!("SPEAKER_{:02}", i % 5)),
            text: format!("Sentence number {i}"),
            start_time: i as f64 * 2.0,
            end_time: (i as f64 * 2.0) + 1.8,
            words: vec![Word {
                word: format!("word_{i}"),
                start: i as f64 * 2.0,
                end: (i as f64 * 2.0) + 1.8,
                p: 0.99,
            }],
        })
        .collect();

    let result: TranscriptionResult = segments.clone().into_iter().collect();

    assert_eq!(result.segments.len(), count);
    for (i, seg) in result.segments.iter().enumerate() {
        assert_eq!(seg.text, format!("Sentence number {i}"));
        assert_eq!(seg.speaker, Some(format!("SPEAKER_{:02}", i % 5)));
        assert_eq!(seg.start_time, i as f64 * 2.0);
        assert_eq!(seg.end_time, (i as f64 * 2.0) + 1.8);
        assert_eq!(seg.words.len(), 1);
        assert_eq!(seg.words[0].word, format!("word_{i}"));
    }

    let speakers = result.speakers.expect("speakers must be populated");
    assert_eq!(speakers.len(), 5);
}
