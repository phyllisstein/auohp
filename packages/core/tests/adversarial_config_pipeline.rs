//! Standalone adversarial stress test harness for Milestone 3:
//! Config Parsing (`config.rs`) & Pipeline Word/Segment Assembly Dataflow (`pipeline.rs`).
//!
//! Empirical challenge verification:
//! 1. `TranscribeConfig::from_str` with malformed JSON, empty string, trailing commas,
//!    unexpected fields, and extreme numerical values.
//! 2. Partial JSON parsing with defaults (`{}`, sparse sub-objects).
//! 3. `FromStr` and `Display` roundtrip property verification across combinatorial parameters.
//! 4. Stress-test word and segment assembly with large transcripts (10,000 to 100,000 words).
//! 5. Verify zero crashes, memory safety, and panic-freedom.

use std::str::FromStr;
use auohp_core::transcription::{
    dominant_speaker, AudioConfig, DecodeConfig, DiarizeConfig, DiarizedSegment, Interpolation,
    Segment, TranscribeConfig, TranscriptionConfig, TranscriptionResult, VadConfig, Word,
};

pub mod types {
    pub use auohp_core::transcription::Word;
}

#[allow(dead_code)]
#[path = "../src/transcription/whisper"]
mod whisper {
    pub mod alignment;
}
use whisper::alignment::assemble_words;

// =========================================================================
// 1. TranscribeConfig Malformed JSON & Rejection Tests
// =========================================================================

#[test]
fn test_config_from_str_empty_string_rejected() {
    let err = TranscribeConfig::from_str("").unwrap_err();
    assert!(
        err.is_eof() || err.is_syntax(),
        "Empty string must return syntax/EOF error, got: {err}"
    );
}

#[test]
fn test_config_from_str_whitespace_only_rejected() {
    for ws in [" ", "\t", "\n", "\r\n", "   \t\n  \r  "] {
        let res = ws.parse::<TranscribeConfig>();
        assert!(res.is_err(), "Whitespace string '{ws:?}' must be rejected");
    }
}

#[test]
fn test_config_from_str_trailing_commas_rejected() {
    let cases = [
        r#"{"audio": {},}"#,
        r#"{"audio": {"resample_chunk": 4096,}}"#,
        r#"{"diarize": {"max_speakers": 2,},}"#,
        r#"[1, 2,]"#,
    ];

    for case in cases {
        let res = case.parse::<TranscribeConfig>();
        assert!(
            res.is_err(),
            "JSON with trailing comma must fail standard parsing: {case}"
        );
    }
}

#[test]
fn test_config_from_str_malformed_syntax_rejected() {
    let malformed_inputs = [
        "{",
        "}",
        r#"{"audio": "#,
        r#"{"audio": {"resample_chunk": 4096"#,
        r#"{"diarize": "max_speakers": 2}"#,
        r#"{audio: {}}"#,
        r#"{'audio': {}}"#,
        r#"{"audio": undefined}"#,
        r#"{"decode": {"beam_size": 5}} trailing_garbage"#,
        r#"{"vad": {"enabled": truthy}}"#,
    ];

    for input in malformed_inputs {
        let res = input.parse::<TranscribeConfig>();
        assert!(
            res.is_err(),
            "Malformed JSON must be rejected gracefully: {input}"
        );
    }
}

#[test]
fn test_config_from_str_non_object_root_rejected() {
    let primitives = [
        "null",
        "true",
        "false",
        "123",
        "-45.67",
        r#""string_value""#,
        "[1, 2, 3]",
    ];

    for prim in primitives {
        let res = prim.parse::<TranscribeConfig>();
        assert!(
            res.is_err(),
            "Non-object root JSON must be rejected: {prim}"
        );
    }
}

/// Serde's struct deserializer implements `visit_seq` as well as `visit_map`.
/// With `#[serde(default)]`, an empty sequence `[]` provides zero elements,
/// causing all struct fields to be populated from `TranscribeConfig::default()`.
#[test]
fn test_config_from_str_empty_array_sequence_defaults() {
    let parsed: TranscribeConfig = "[]".parse().expect("Empty sequence parses via visit_seq defaults");
    assert_eq!(parsed, TranscribeConfig::default());
}

#[test]
fn test_config_from_str_type_mismatches_rejected() {
    let type_errors = [
        (r#"{"audio": "not_an_object"}"#, "string instead of AudioConfig"),
        (r#"{"audio": 12345}"#, "number instead of AudioConfig"),
        (r#"{"audio": ["resample_chunk", 4096]}"#, "array instead of AudioConfig"),
        (r#"{"audio": {"interpolation": "hyperbolic"}}"#, "invalid enum variant"),
        (r#"{"audio": {"interpolation": 1}}"#, "number instead of enum variant"),
        (r#"{"audio": {"resample_chunk": "4096"}}"#, "string instead of usize"),
        (r#"{"audio": {"resample_chunk": -1}}"#, "negative number for usize"),
        (r#"{"vad": {"enabled": "yes"}}"#, "string instead of bool"),
        (r#"{"vad": {"enabled": 1}}"#, "int instead of bool"),
        (r#"{"decode": {"beam_size": "five"}}"#, "string instead of i32"),
        (r#"{"decode": {"token_timestamps": 1}}"#, "int instead of bool"),
        (r#"{"diarize": {"max_speakers": -2}}"#, "negative int for usize"),
        (r#"{"diarize": {"max_speakers": 3.14159}}"#, "float for usize"),
    ];

    for (json, desc) in type_errors {
        let res = json.parse::<TranscribeConfig>();
        assert!(
            res.is_err(),
            "Type mismatch should fail parsing ({desc}): {json}"
        );
    }
}

// =========================================================================
// 2. Partial JSON Parsing with Defaults Tests
// =========================================================================

#[test]
fn test_config_empty_object_yields_full_defaults() {
    let empty_json = "{}";
    let parsed: TranscribeConfig = empty_json.parse().expect("Empty object should parse cleanly");
    let default_cfg = TranscribeConfig::default();

    assert_eq!(
        parsed, default_cfg,
        "Empty object '{{}}' must match TranscribeConfig::default()"
    );
}

#[test]
fn test_config_sparse_sub_objects_yield_defaults() {
    let sparse_cases = [
        r#"{"audio": {}}"#,
        r#"{"decode": {}}"#,
        r#"{"vad": {}}"#,
        r#"{"diarize": {}}"#,
        r#"{"audio": {}, "decode": {}, "vad": {}, "diarize": {}}"#,
    ];

    let default_cfg = TranscribeConfig::default();
    for case in sparse_cases {
        let parsed: TranscribeConfig = case
            .parse()
            .unwrap_or_else(|e| panic!("Failed to parse {case}: {e}"));
        assert_eq!(
            parsed, default_cfg,
            "Sparse section object must equal default: {case}"
        );
    }
}

#[test]
fn test_config_deeply_nested_single_field_overrides() {
    // 1. Audio override
    let json_audio = r#"{"audio": {"resample_chunk": 8192, "interpolation": "cubic"}}"#;
    let cfg = json_audio.parse::<TranscribeConfig>().unwrap();
    assert_eq!(cfg.audio.resample_chunk, 8192);
    assert_eq!(cfg.audio.interpolation, Interpolation::Cubic);
    assert_eq!(cfg.audio.sinc_len, 256);
    assert_eq!(cfg.audio.f_cutoff, 0.95);
    assert_eq!(cfg.decode, DecodeConfig::default());
    assert_eq!(cfg.vad, VadConfig::default());
    assert_eq!(cfg.diarize, DiarizeConfig::default());

    // 2. Vad override
    let json_vad = r#"{"vad": {"enabled": false, "threshold": 0.65}}"#;
    let cfg_vad = json_vad.parse::<TranscribeConfig>().unwrap();
    assert!(!cfg_vad.vad.enabled);
    assert_eq!(cfg_vad.vad.threshold, Some(0.65));
    assert_eq!(cfg_vad.vad.max_speech_duration_s, Some(60.0));
    assert_eq!(cfg_vad.audio, AudioConfig::default());

    // 3. Decode override
    let json_decode = r#"{"decode": {"beam_size": 1, "patience": 2.5, "language": "de"}}"#;
    let cfg_decode = json_decode.parse::<TranscribeConfig>().unwrap();
    assert_eq!(cfg_decode.decode.beam_size, 1);
    assert_eq!(cfg_decode.decode.patience, 2.5);
    assert_eq!(cfg_decode.decode.language, Some("de".to_string()));
    assert!(cfg_decode.decode.no_context);
    assert!(cfg_decode.decode.token_timestamps);

    // 4. Diarize override
    let json_diarize = r#"{"diarize": {"max_speakers": 6}}"#;
    let cfg_diarize = json_diarize.parse::<TranscribeConfig>().unwrap();
    assert_eq!(cfg_diarize.diarize.max_speakers, 6);
    assert!(cfg_diarize.diarize.enabled);
}

// =========================================================================
// 3. Unexpected & Unknown Fields (Forward Compatibility)
// =========================================================================

#[test]
fn test_config_ignores_unknown_fields_gracefully() {
    let json_with_extra = r#"{
        "unknown_top_field": 42,
        "audio": {
            "resample_chunk": 2048,
            "future_filter_mode": "chebyshev"
        },
        "experimental_flags": {
            "fast_vad": true
        },
        "diarize": {
            "max_speakers": 3,
            "custom_metric": "mahalanobis"
        }
    }"#;

    let cfg: TranscribeConfig = json_with_extra
        .parse()
        .expect("Serde should ignore unexpected fields by default");
    assert_eq!(cfg.audio.resample_chunk, 2048);
    assert_eq!(cfg.diarize.max_speakers, 3);
    assert!(cfg.diarize.enabled);
}

// =========================================================================
// 4. Extreme Values & Boundary Conditions
// =========================================================================

#[test]
fn test_config_extreme_integers() {
    // max_speakers = 0
    let json_zero = r#"{"diarize": {"max_speakers": 0}}"#;
    let cfg = json_zero.parse::<TranscribeConfig>().unwrap();
    assert_eq!(cfg.diarize.max_speakers, 0);

    // max_speakers = usize::MAX
    let max_usize_str = format!(r#"{{"diarize": {{"max_speakers": {}}}}}"#, usize::MAX);
    let cfg_max = max_usize_str.parse::<TranscribeConfig>().unwrap();
    assert_eq!(cfg_max.diarize.max_speakers, usize::MAX);

    // max_speakers integer overflow (> usize::MAX)
    let overflow_usize = format!(r#"{{"diarize": {{"max_speakers": {}0}}}}"#, usize::MAX);
    assert!(overflow_usize.parse::<TranscribeConfig>().is_err());

    // beam_size negative and i32::MAX
    let json_beam_neg = r#"{"decode": {"beam_size": -10}}"#;
    let cfg_neg = json_beam_neg.parse::<TranscribeConfig>().unwrap();
    assert_eq!(cfg_neg.decode.beam_size, -10);

    let json_beam_max = format!(r#"{{"decode": {{"beam_size": {}}}}}"#, i32::MAX);
    let cfg_beam_max = json_beam_max.parse::<TranscribeConfig>().unwrap();
    assert_eq!(cfg_beam_max.decode.beam_size, i32::MAX);

    let overflow_i32 = format!(r#"{{"decode": {{"beam_size": {}0}}}}"#, i32::MAX as i64 + 1);
    assert!(overflow_i32.parse::<TranscribeConfig>().is_err());
}

#[test]
fn test_config_extreme_floats() {
    let json_floats = r#"{
        "audio": {
            "f_cutoff": 0.0000001
        },
        "decode": {
            "patience": 1e10,
            "entropy_thold": -99.5,
            "logprob_thold": -1000.0
        },
        "vad": {
            "threshold": 0.0,
            "max_speech_duration_s": 86400.0
        }
    }"#;

    let cfg: TranscribeConfig = json_floats.parse().unwrap();
    assert_eq!(cfg.audio.f_cutoff, 0.0000001);
    assert_eq!(cfg.decode.patience, 1e10);
    assert_eq!(cfg.decode.entropy_thold, -99.5);
    assert_eq!(cfg.decode.logprob_thold, Some(-1000.0));
    assert_eq!(cfg.vad.threshold, Some(0.0));
    assert_eq!(cfg.vad.max_speech_duration_s, Some(86400.0));
}

#[test]
fn test_config_large_strings_and_unicode_initial_prompt() {
    let huge_prompt = "ActUp ".repeat(10_000); // 60,000 chars
    let escaped_prompt = serde_json::to_string(&huge_prompt).unwrap();
    let json = format!(r#"{{"decode": {{"initial_prompt": {escaped_prompt}}}}}"#);

    let cfg: TranscribeConfig = json.parse().unwrap();
    assert_eq!(cfg.decode.initial_prompt.as_ref().unwrap().len(), 60_000);

    // Prompt with complex Unicode and emoji
    let unicode_prompt = "Silence=Death ✊🏽 ACT UP / New York: Larry Kramer & Ann Northrop — 1987";
    let escaped_unicode = serde_json::to_string(unicode_prompt).unwrap();
    let json_uni = format!(r#"{{"decode": {{"initial_prompt": {escaped_unicode}}}}}"#);
    let cfg_uni: TranscribeConfig = json_uni.parse().unwrap();
    assert_eq!(cfg_uni.decode.initial_prompt.unwrap(), unicode_prompt);
}

// =========================================================================
// 5. FromStr and Display Roundtrip Property
// =========================================================================

#[test]
fn test_config_fromstr_display_roundtrip_default() {
    let config = TranscribeConfig::default();
    let displayed = config.to_string();
    let parsed: TranscribeConfig = displayed.parse().expect("Displayed JSON must parse cleanly");
    assert_eq!(
        config, parsed,
        "Default config must roundtrip through Display and FromStr"
    );
}

#[test]
fn test_config_fromstr_display_roundtrip_combinatorial() {
    let interpolations = [
        Interpolation::Nearest,
        Interpolation::Linear,
        Interpolation::Quadratic,
        Interpolation::Cubic,
    ];

    for interp in interpolations {
        for enabled in [true, false] {
            for max_speakers in [1, 2, 4, 16] {
                let config = TranscribeConfig {
                    audio: AudioConfig {
                        resample_chunk: 2048,
                        sinc_len: 128,
                        f_cutoff: 0.92,
                        oversampling_factor: 128,
                        interpolation: interp,
                    },
                    decode: DecodeConfig {
                        language: Some("fr".into()),
                        beam_size: 3,
                        patience: 1.5,
                        entropy_thold: 2.8,
                        logprob_thold: Some(-1.2),
                        no_speech_thold: Some(0.4),
                        temperature: Some(0.2),
                        temperature_inc: Some(0.1),
                        no_context: false,
                        suppress_nst: Some(true),
                        initial_prompt: Some("AIDS Coalition to Unleash Power".into()),
                        max_len: Some(120),
                        split_on_word: Some(true),
                        token_timestamps: false,
                    },
                    vad: VadConfig {
                        enabled,
                        threshold: Some(0.55),
                        min_speech_duration_ms: Some(250),
                        min_silence_duration_ms: Some(100),
                        max_speech_duration_s: Some(45.0),
                        speech_pad_ms: Some(30),
                        samples_overlap_s: Some(0.25),
                    },
                    diarize: DiarizeConfig {
                        enabled,
                        max_speakers,
                    },
                };

                let serialized = config.to_string();
                let parsed: TranscribeConfig = serialized
                    .parse()
                    .unwrap_or_else(|e| panic!("Roundtrip parse failed for {serialized}: {e}"));
                assert_eq!(config, parsed, "Config must exactly roundtrip");

                // Multi-hop roundtrip idempotence: config -> s1 -> config2 -> s2 -> config3
                let s2 = parsed.to_string();
                let parsed2: TranscribeConfig = s2.parse().unwrap();
                assert_eq!(parsed, parsed2);
                assert_eq!(serialized, s2);
            }
        }
    }
}

#[test]
fn test_config_display_failure_mode_with_nan_handling() {
    let mut config = TranscribeConfig::default();
    config.audio.f_cutoff = f32::NAN;

    // serde_json serializes f32::NAN as null in standard JSON format.
    let display_str = config.to_string();
    assert!(
        display_str.contains(r#""f_cutoff":null"#),
        "serde_json serializes f32::NAN as null, got: {display_str}"
    );

    // Because f_cutoff is f32 (not Option<f32>), parsing the displayed JSON
    // with null returns an Err(invalid type: null, expected f32) without panicking.
    let reparsed = display_str.parse::<TranscribeConfig>();
    assert!(
        reparsed.is_err(),
        "Parsing JSON with 'f_cutoff: null' must return Err, not panic"
    );
}

// =========================================================================
// 6. Pipeline Word & Segment Assembly Stress (10,000+ Words)
// =========================================================================

#[test]
fn test_pipeline_assembly_10k_words_single_segment() {
    const WORD_COUNT: usize = 10_000;
    let mut words = Vec::with_capacity(WORD_COUNT);
    let mut current_time = 0.0f64;

    for i in 0..WORD_COUNT {
        let start = current_time;
        let end = current_time + 0.25;
        words.push(Word {
            word: format!("word_{i}"),
            start,
            end,
            p: 0.95,
        });
        current_time = end + 0.05;
    }

    let diarized = vec![
        DiarizedSegment {
            speaker: "SPEAKER_00".to_string(),
            start: 0.0,
            end: current_time,
        },
    ];

    // Simulate whisper segment
    let seg_start = words.first().unwrap().start;
    let seg_end = words.last().unwrap().end;
    let seg_text = format!("Massive monologue containing {} words", WORD_COUNT);

    // Map through pipeline assembly logic
    let speaker_attr = dominant_speaker(seg_start, seg_end, &diarized).map(str::to_owned);
    assert_eq!(speaker_attr.as_deref(), Some("SPEAKER_00"));

    let segment = Segment {
        speaker: speaker_attr,
        text: seg_text,
        start_time: seg_start,
        end_time: seg_end,
        words,
    };

    // Assemble into TranscriptionResult via FromIterator
    let result: TranscriptionResult = vec![segment].into_iter().collect();

    assert_eq!(result.segments.len(), 1);
    assert_eq!(result.segments[0].words.len(), WORD_COUNT);
    assert_eq!(result.segments[0].words[0].word, "word_0");
    assert_eq!(result.segments[0].words[WORD_COUNT - 1].word, format!("word_{}", WORD_COUNT - 1));
    assert_eq!(result.speakers, Some(vec!["SPEAKER_00".to_string()]));
    assert!(result.models.is_some());

    // Verify JSON roundtrip of massive TranscriptionResult
    let json = serde_json::to_string(&result).expect("Result should serialize cleanly");
    assert!(json.len() > 200_000, "JSON payload should reflect 10k words");
    let deserialized: TranscriptionResult = serde_json::from_str(&json).expect("Result should deserialize cleanly");
    assert_eq!(deserialized.segments[0].words.len(), WORD_COUNT);
}

#[test]
fn test_pipeline_assembly_10k_words_multispeaker_alternating_turns() {
    const SEGMENTS_COUNT: usize = 2_500;
    const WORDS_PER_SEG: usize = 4;
    // Total words: 2,500 * 4 = 10,000 words.

    let mut diarized = Vec::with_capacity(SEGMENTS_COUNT);
    let mut whisper_segments = Vec::with_capacity(SEGMENTS_COUNT);

    let mut t = 0.0f64;
    for s in 0..SEGMENTS_COUNT {
        let speaker = if s % 2 == 0 { "SPEAKER_00" } else { "SPEAKER_01" };
        let seg_start = t;
        let mut words = Vec::with_capacity(WORDS_PER_SEG);

        for w in 0..WORDS_PER_SEG {
            let w_start = t;
            let w_end = t + 0.3;
            words.push(Word {
                word: format!("s{s}_w{w}"),
                start: w_start,
                end: w_end,
                p: 0.90,
            });
            t = w_end + 0.05;
        }
        let seg_end = t;
        t += 0.2; // Pause between segments

        diarized.push(DiarizedSegment {
            speaker: speaker.to_string(),
            start: seg_start,
            end: seg_end,
        });

        whisper_segments.push((
            speaker,
            format!("Segment {s} text"),
            seg_start,
            seg_end,
            words,
        ));
    }

    // Mirror pipeline.rs segment assembly using into_iter zero-copy pattern
    let segments: Vec<Segment> = whisper_segments
        .into_iter()
        .map(|(_expected_spk, text, start, end, words)| Segment {
            speaker: dominant_speaker(start, end, &diarized).map(str::to_owned),
            text,
            start_time: start,
            end_time: end,
            words,
        })
        .collect();

    // Verify all 2,500 segments were correctly speaker-attributed
    for (i, seg) in segments.iter().enumerate() {
        let expected = if i % 2 == 0 { "SPEAKER_00" } else { "SPEAKER_01" };
        assert_eq!(seg.speaker.as_deref(), Some(expected));
        assert_eq!(seg.words.len(), WORDS_PER_SEG);
    }

    // Convert into TranscriptionResult
    let result: TranscriptionResult = segments.into();
    assert_eq!(result.segments.len(), SEGMENTS_COUNT);

    let total_words: usize = result.segments.iter().map(|s| s.words.len()).sum();
    assert_eq!(total_words, 10_000);

    let speakers = result.speakers.expect("speakers should be populated");
    assert_eq!(speakers.len(), 2);
    assert!(speakers.contains(&"SPEAKER_00".to_string()));
    assert!(speakers.contains(&"SPEAKER_01".to_string()));
}

#[test]
fn test_pipeline_assembly_100k_words_extreme_stress() {
    const SEGMENTS_COUNT: usize = 10_000;
    const WORDS_PER_SEG: usize = 10;
    // Total words: 10,000 * 10 = 100,000 words!

    let start_instant = std::time::Instant::now();

    let mut segments = Vec::with_capacity(SEGMENTS_COUNT);
    let mut current_time = 0.0f64;

    for s in 0..SEGMENTS_COUNT {
        let seg_start = current_time;
        let mut words = Vec::with_capacity(WORDS_PER_SEG);
        for w in 0..WORDS_PER_SEG {
            let start = current_time;
            let end = current_time + 0.2;
            words.push(Word {
                word: format!("token_{s}_{w}"),
                start,
                end,
                p: 0.99,
            });
            current_time = end + 0.02;
        }
        let seg_end = current_time;
        current_time += 0.1;

        let speaker_str = format!("SPEAKER_{:02}", s % 5);
        segments.push(Segment {
            speaker: Some(speaker_str),
            text: format!("Segment {s}"),
            start_time: seg_start,
            end_time: seg_end,
            words,
        });
    }

    // Assemble via FromIterator
    let result: TranscriptionResult = segments.into_iter().collect();

    let elapsed = start_instant.elapsed();
    assert_eq!(result.segments.len(), SEGMENTS_COUNT);
    let total_words: usize = result.segments.iter().map(|s| s.words.len()).sum();
    assert_eq!(total_words, 100_000);

    let speakers = result.speakers.unwrap();
    assert_eq!(speakers.len(), 5);
    for spk_id in 0..5 {
        assert!(speakers.contains(&format!("SPEAKER_{:02}", spk_id)));
    }

    // Verify fast execution: 100k words should assemble and collect in well under 1 second
    assert!(
        elapsed.as_millis() < 1000,
        "100k words assembly took too long: {} ms",
        elapsed.as_millis()
    );
}

#[test]
fn test_pipeline_assembly_with_unmatched_diarization_gaps() {
    // Segments with silence/gaps where diarization has no match
    let diarized = vec![
        DiarizedSegment {
            speaker: "SPEAKER_00".to_string(),
            start: 10.0,
            end: 20.0,
        },
    ];

    let segments = vec![
        // Unmatched segment before diarized speech
        Segment {
            speaker: dominant_speaker(0.0, 5.0, &diarized).map(str::to_owned),
            text: "Unmatched intro".into(),
            start_time: 0.0,
            end_time: 5.0,
            words: vec![Word { word: "intro".into(), start: 1.0, end: 2.0, p: 0.5 }],
        },
        // Matched segment
        Segment {
            speaker: dominant_speaker(10.0, 18.0, &diarized).map(str::to_owned),
            text: "Matched speech".into(),
            start_time: 10.0,
            end_time: 18.0,
            words: vec![Word { word: "speech".into(), start: 11.0, end: 12.0, p: 0.9 }],
        },
        // Unmatched segment after diarized speech
        Segment {
            speaker: dominant_speaker(25.0, 30.0, &diarized).map(str::to_owned),
            text: "Unmatched outro".into(),
            start_time: 25.0,
            end_time: 30.0,
            words: vec![Word { word: "outro".into(), start: 26.0, end: 27.0, p: 0.6 }],
        },
    ];

    let result: TranscriptionResult = segments.into();
    assert_eq!(result.segments[0].speaker, None);
    assert_eq!(result.segments[1].speaker.as_deref(), Some("SPEAKER_00"));
    assert_eq!(result.segments[2].speaker, None);

    // Speakers list should only contain "SPEAKER_00" and ignore None
    let speakers = result.speakers.expect("speakers vector should exist");
    assert_eq!(speakers, vec!["SPEAKER_00".to_string()]);
}

#[test]
fn test_pipeline_assembly_empty_segments_list() {
    let segments: Vec<Segment> = Vec::new();
    let result: TranscriptionResult = segments.into();

    assert!(result.segments.is_empty());
    assert_eq!(result.speakers, Some(Vec::new()));
    assert!(result.models.is_some());
}

#[test]
fn test_transcription_config_from_json_and_from_str() {
    let json = r#"{"diarize": {"max_speakers": 4}}"#;
    let cfg = TranscribeConfig::from_str(json).unwrap();
    assert_eq!(cfg.diarize.max_speakers, 4);
    assert_eq!(cfg.audio.resample_chunk, 4096);

    let from_json_cfg = TranscribeConfig::from_json(json).unwrap();
    assert_eq!(cfg, from_json_cfg);

    let bad = TranscribeConfig::from_json("invalid");
    assert!(bad.is_err());
}

#[test]
fn test_assemble_words_10k_tokens_stress() {
    const TOKEN_COUNT: usize = 10_000;
    let mut tokens: Vec<(&'static [u8], f64, f32)> = Vec::with_capacity(TOKEN_COUNT);

    let start_instant = std::time::Instant::now();
    for i in 0..TOKEN_COUNT {
        let t = i as f64 * 0.05;
        let p = 0.90 + (i % 10) as f32 * 0.01;
        // Interleave space-prefixed tokens and continuation tokens
        if i % 3 == 0 {
            tokens.push((b" ACT", t, p));
        } else if i % 3 == 1 {
            tokens.push((b"-UP", t, p));
        } else {
            tokens.push((b" history", t, p));
        }
    }

    let words = assemble_words(tokens);
    let elapsed = start_instant.elapsed();

    assert!(
        !words.is_empty(),
        "Words list must be populated from 10k tokens"
    );
    // 10,000 tokens should produce roughly ~6,666 words and take < 50ms
    assert!(
        elapsed.as_millis() < 200,
        "10k tokens assemble_words took too long: {} ms",
        elapsed.as_millis()
    );
}

// =========================================================================
// 7. Public API Surface & Interchangeability: TranscriptionConfig & TranscribeConfig
// =========================================================================

#[test]
fn test_transcription_config_and_transcribe_config_type_identity() {
    use std::any::TypeId;
    assert_eq!(
        TypeId::of::<TranscriptionConfig>(),
        TypeId::of::<TranscribeConfig>(),
        "TranscriptionConfig must be a type alias for TranscribeConfig"
    );
}

#[test]
fn test_transcription_config_and_transcribe_config_interchangeable_usage() {
    fn consume_transcribe_config(cfg: &TranscribeConfig) -> usize {
        cfg.audio.resample_chunk
    }

    fn consume_transcription_config(cfg: &TranscriptionConfig) -> usize {
        cfg.audio.resample_chunk
    }

    fn mutate_transcribe_config(cfg: &mut TranscribeConfig) {
        cfg.audio.resample_chunk = 8192;
    }

    fn mutate_transcription_config(cfg: &mut TranscriptionConfig) {
        cfg.audio.resample_chunk = 16384;
    }

    // Instantiation and mutual assignment
    let mut config_a: TranscriptionConfig = TranscribeConfig::default();
    let mut config_b: TranscribeConfig = TranscriptionConfig::default();

    assert_eq!(config_a, config_b);

    // Pass TranscriptionConfig to fn taking TranscribeConfig
    assert_eq!(consume_transcribe_config(&config_a), 4096);
    // Pass TranscribeConfig to fn taking TranscriptionConfig
    assert_eq!(consume_transcription_config(&config_b), 4096);

    // Cross-mutation
    mutate_transcribe_config(&mut config_a);
    assert_eq!(config_a.audio.resample_chunk, 8192);

    mutate_transcription_config(&mut config_b);
    assert_eq!(config_b.audio.resample_chunk, 16384);

    // Direct assignment across alias names
    config_a = config_b.clone();
    assert_eq!(config_a.audio.resample_chunk, 16384);

    // Slice and container equivalence
    let list: Vec<TranscriptionConfig> = vec![config_a];
    let slice: &[TranscribeConfig] = &list;
    assert_eq!(slice.len(), 1);
    assert_eq!(slice[0].audio.resample_chunk, 16384);
}

#[test]
fn test_transcription_config_from_json_and_from_str_methods() {
    let json = r#"{"decode": {"beam_size": 7}}"#;
    let cfg: TranscriptionConfig = json.parse().unwrap();
    assert_eq!(cfg.decode.beam_size, 7);

    let from_json_cfg = TranscriptionConfig::from_json(json).unwrap();
    assert_eq!(cfg, from_json_cfg);

    let displayed = cfg.to_string();
    let parsed_back: TranscribeConfig = displayed.parse().unwrap();
    assert_eq!(parsed_back, cfg);
}

#[test]
fn test_pipeline_run_with_accepts_transcription_config_reference() {
    use auohp_core::transcription::run_with;
    use std::path::Path;

    let cfg: TranscriptionConfig = TranscriptionConfig::default();
    // Non-existent path returns Err, proving run_with signature accepts &TranscriptionConfig
    let res = run_with(Path::new("/nonexistent/audio/file.wav"), &cfg);
    assert!(res.is_err(), "Nonexistent file must error, verifying run_with signature");
}

#[test]
fn test_public_facade_constants_and_model_exports() {
    use auohp_core::transcription::{
        EMBEDDING_MODEL_FILE, SEGMENTATION_MODEL_FILE, VAD_MODEL_FILE, WHISPER_MODEL_FILE,
    };

    assert_eq!(WHISPER_MODEL_FILE, "ggml-large-v3.bin");
    assert_eq!(VAD_MODEL_FILE, "ggml-silero-v6.2.0.bin");
    assert_eq!(SEGMENTATION_MODEL_FILE, "pyannote-segmentation-3.0.onnx");
    assert_eq!(EMBEDDING_MODEL_FILE, "wespeaker_en_voxceleb_ECAPA1024.onnx");
}


