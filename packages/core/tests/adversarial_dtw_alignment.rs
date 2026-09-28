//! Standalone adversarial stress test harness for DTW Word Assembly & Byte-level BPE (`whisper/alignment.rs`).
//!
//! Empirical challenge verification for Milestone 2:
//! 1. UTF-8 multi-byte split across arbitrary token boundaries (2, 3, 4 bytes, emojis, CJK, accents).
//! 2. Incomplete or malformed UTF-8 byte sequences at segment boundaries.
//! 3. Consecutive space-prefixed tokens, tokens with only spaces, empty token lists, empty byte slices.
//! 4. Timestamps with extreme floating point values, zero, subnormal numbers, NaN, and infinity.
//! 5. Dialogue dash stripping (`-`, `--`, `---`, unicode em-dashes).
//! 6. Byte-level BPE safety (zero U+FFFD replacement character corruption on valid text).
//! 7. Word interval validity and timestamp monotonicity.

pub mod types {
    pub use auohp_core::transcription::Word;
}

#[allow(dead_code)]
#[path = "../src/transcription/whisper"]
mod whisper {
    pub mod alignment;
}

use whisper::alignment::{assemble_words, round_to, strip_turn_dash};
use auohp_core::transcription::Word;

// ── Word collection oracle mirroring collect_words downstream logic ─────────

/// Mirrors the word grouping, turn dash removal, and interval construction in
/// `collect_words` after `assemble_words` has completed.
fn finalize_words(groups: Vec<(String, f64, f32)>, seg_end: f64) -> Vec<Word> {
    let mut groups = groups;
    if groups.first().is_some_and(|(w, _, _)| w == "-") {
        groups.remove(0);
    }

    groups
        .iter()
        .enumerate()
        .filter(|(_, (w, _, _))| !w.is_empty())
        .map(|(i, (word, start, p))| {
            let end = groups
                .get(i + 1)
                .map(|(_, next_start, _)| *next_start)
                .unwrap_or(seg_end)
                .max(*start);
            Word {
                word: word.clone(),
                start: *start,
                end,
                p: round_to(*p as f64, 2) as f32,
            }
        })
        .collect()
}

// ── UTF-8 Multi-byte Splitting & BPE Safety ──────────────────────────────────

#[test]
fn test_utf8_2byte_characters_split_at_all_positions() {
    // 2-byte characters: é (0xC3, 0xA9), ñ (0xC3, 0xB1), α (0xCE, 0xB1), д (0xD0, 0xB4)
    let chars = ["café", "año", "πατάτα", "город"];
    for s in chars {
        for ch in s.chars() {
            let mut buf = [0u8; 4];
            let encoded = ch.encode_utf8(&mut buf);
            if encoded.len() == 2 {
                let b = encoded.as_bytes();
                // Split 1: 1 byte + 1 byte
                let words = assemble_words(vec![
                    (&b[..1], 1.0, 0.9),
                    (&b[1..], 1.1, 0.85),
                ]);
                assert_eq!(words.len(), 1, "Must reassemble into 1 word");
                assert_eq!(words[0].0, ch.to_string(), "Must match char exactly");
                assert!(!words[0].0.contains('\u{FFFD}'), "No U+FFFD replacement corruption");
                assert_eq!(words[0].1, 1.0, "Start timestamp preserved from first token");
                assert_eq!(words[0].2, 0.85, "Weakest confidence wins");
            }
        }
    }
}

#[test]
fn test_utf8_3byte_characters_split_permutations() {
    // 3-byte characters: U+2019 (’), U+20AC (€), U+4E2D (中), U+6587 (文), U+0950 (ॐ)
    let test_chars = ['’', '€', '中', '文', 'ॐ'];
    for ch in test_chars {
        let mut buf = [0u8; 4];
        let encoded = ch.encode_utf8(&mut buf);
        let b = encoded.as_bytes();
        assert_eq!(b.len(), 3);

        // Partition 1: 1 byte + 2 bytes
        let w1 = assemble_words(vec![
            (&b[..1], 1.0, 0.9),
            (&b[1..], 1.1, 0.7),
        ]);
        assert_eq!(w1.len(), 1);
        assert_eq!(w1[0].0, ch.to_string());
        assert!(!w1[0].0.contains('\u{FFFD}'));
        assert_eq!(w1[0].2, 0.7);

        // Partition 2: 2 bytes + 1 byte
        let w2 = assemble_words(vec![
            (&b[..2], 2.0, 0.6),
            (&b[2..], 2.1, 0.95),
        ]);
        assert_eq!(w2.len(), 1);
        assert_eq!(w2[0].0, ch.to_string());
        assert!(!w2[0].0.contains('\u{FFFD}'));
        assert_eq!(w2[0].2, 0.6);

        // Partition 3: 1 byte + 1 byte + 1 byte
        let w3 = assemble_words(vec![
            (&b[..1], 3.0, 0.9),
            (&b[1..2], 3.1, 0.5),
            (&b[2..], 3.2, 0.8),
        ]);
        assert_eq!(w3.len(), 1);
        assert_eq!(w3[0].0, ch.to_string());
        assert!(!w3[0].0.contains('\u{FFFD}'));
        assert_eq!(w3[0].2, 0.5);
    }
}

#[test]
fn test_utf8_4byte_emojis_and_symbols_split_permutations() {
    // 4-byte characters: 🎉 (U+1F389), 🦀 (U+1F980), 𝄞 (U+1D11E), 🏳 (U+1F3F3)
    let test_chars = ['🎉', '🦀', '𝄞', '🏳'];
    for ch in test_chars {
        let mut buf = [0u8; 4];
        let encoded = ch.encode_utf8(&mut buf);
        let b = encoded.as_bytes();
        assert_eq!(b.len(), 4);

        // All non-empty partitions of 4 bytes:
        // [1, 3], [2, 2], [3, 1], [1, 1, 2], [1, 2, 1], [2, 1, 1], [1, 1, 1, 1]
        let partitions: Vec<Vec<usize>> = vec![
            vec![1, 3],
            vec![2, 2],
            vec![3, 1],
            vec![1, 1, 2],
            vec![1, 2, 1],
            vec![2, 1, 1],
            vec![1, 1, 1, 1],
        ];

        for part in partitions {
            let mut toks = Vec::new();
            let mut offset = 0;
            let mut t = 1.0;
            for len in part {
                toks.push((&b[offset..offset + len], t, 0.8f32));
                offset += len;
                t += 0.1;
            }

            let words = assemble_words(toks);
            assert_eq!(words.len(), 1, "Failed for partition of char {ch}");
            assert_eq!(words[0].0, ch.to_string(), "Character mismatch for {ch}");
            assert!(
                !words[0].0.contains('\u{FFFD}'),
                "Found corruption in 4-byte char {ch}"
            );
        }
    }
}

#[test]
fn test_complex_fuzzed_utf8_fragment_generator() {
    // A complex multi-lingual string with 1-byte, 2-byte, 3-byte, and 4-byte code points
    let corpus = "ACT UP: Silence=Death! 1989年 ¡No pasarán! Café résumé 🦀🏳️‍🌈 Zürich, 100€.";
    let bytes = corpus.as_bytes();

    // Pseudo-random deterministic chunking (simulating varied BPE token splits)
    let mut rng_state: u64 = 0x123456789ABCDEF0;
    let mut next_rnd = || -> usize {
        rng_state = rng_state.wrapping_mul(6364136223846793005).wrapping_add(1);
        ((rng_state >> 33) % 4) as usize + 1 // 1 to 4 bytes per chunk
    };

    let mut start = 0;
    let mut chunks: Vec<&[u8]> = Vec::new();
    while start < bytes.len() {
        let step = next_rnd().min(bytes.len() - start);
        chunks.push(&bytes[start..start + step]);
        start += step;
    }

    let mut toks: Vec<(&[u8], f64, f32)> = Vec::new();
    let mut t = 0.0;
    for chunk in chunks {
        toks.push((chunk, t, 0.9));
        t += 0.05;
    }

    let words = assemble_words(toks);
    // Rejoin words with single space (since assemble_words strips the leading space that opened the word)
    let reconstructed = words
        .iter()
        .map(|w| w.0.as_str())
        .collect::<Vec<_>>()
        .join(" ");

    // Verify no replacement character anywhere in valid UTF-8 corpus
    assert!(
        !reconstructed.contains('\u{FFFD}'),
        "Corpus reassembly produced U+FFFD corruption: {reconstructed}"
    );

    // Verify key landmark words assembled intact
    assert!(reconstructed.contains("ACT"));
    assert!(reconstructed.contains("Silence=Death!"));
    assert!(reconstructed.contains("1989年"));
    assert!(reconstructed.contains("¡No"));
    assert!(reconstructed.contains("pasarán!"));
    assert!(reconstructed.contains("Café"));
    assert!(reconstructed.contains("résumé"));
    assert!(reconstructed.contains("🦀"));
    assert!(reconstructed.contains("Zürich,"));
    assert!(reconstructed.contains("100€."));
}

// ── Incomplete & Malformed UTF-8 at Boundaries ───────────────────────────────

#[test]
fn test_incomplete_utf8_at_segment_boundary_never_panics() {
    // Truncated multi-byte UTF-8 sequences at end of segment:
    // 1. Truncated 2-byte: [0xC3]
    let words = assemble_words(vec![(" café".as_bytes(), 0.0, 0.9), (&[0xC3u8] as &[u8], 0.5, 0.8)]);
    assert_eq!(words.len(), 1, "Continuation without space appends to previous word");
    assert!(words[0].0.contains('\u{FFFD}'), "Truncated byte produces replacement character");

    // 2. Truncated 3-byte: [0xE2, 0x80]
    let words2 = assemble_words(vec![(b" word" as &[u8], 1.0, 0.9), (&[0xE2u8, 0x80] as &[u8], 1.5, 0.8)]);
    assert_eq!(words2.len(), 1);
    assert!(words2[0].0.contains('\u{FFFD}'));

    // 3. Truncated 4-byte: [0xF0, 0x9F, 0xA6]
    let words3 = assemble_words(vec![(b" crab" as &[u8], 2.0, 0.9), (&[0xF0u8, 0x9F, 0xA6] as &[u8], 2.5, 0.8)]);
    assert_eq!(words3.len(), 1);
    assert!(words3[0].0.contains('\u{FFFD}'));

    // 4. Standalone truncated token opening a segment
    let words4 = assemble_words(vec![(&[0xF0u8, 0x9F] as &[u8], 0.0, 0.5)]);
    assert_eq!(words4.len(), 1);
    assert_eq!(words4[0].0, "\u{FFFD}");
}

#[test]
fn test_malformed_utf8_bytes_are_handled_safely() {
    // Lone continuation bytes, overlong sequences, and invalid bytes
    let malformed_inputs: Vec<&[u8]> = vec![
        &[0x80u8],             // bare continuation byte
        &[0xBFu8],             // bare continuation byte
        &[0xFFu8],             // invalid byte
        &[0xFEu8],             // invalid byte
        &[0xC0u8, 0x80],       // overlong ASCII NUL
        &[0xE0u8, 0x80, 0x80], // overlong sequence
        &[0xF5u8, 0x80, 0x80, 0x80], // out-of-range codepoint (> U+10FFFF)
    ];

    for bad in malformed_inputs {
        let words = assemble_words(vec![(bad, 0.0, 0.5)]);
        assert_eq!(words.len(), 1);
        assert!(
            words[0].0.contains('\u{FFFD}'),
            "Malformed bytes must yield replacement char"
        );
    }
}

// ── Space Tokens, Empty Lists & Word Boundary Handling ───────────────────────

#[test]
fn test_empty_token_stream() {
    let empty: Vec<(&[u8], f64, f32)> = Vec::new();
    let words = assemble_words(empty);
    assert!(words.is_empty(), "Empty token list must yield empty output");

    let final_words = finalize_words(words, 10.0);
    assert!(final_words.is_empty());
}

#[test]
fn test_empty_byte_slice_tokens() {
    // Tokens with empty byte slices
    let words = assemble_words(vec![
        (b"" as &[u8], 0.0, 0.9),
        (b"" as &[u8], 0.5, 0.8),
    ]);
    assert_eq!(words.len(), 1);
    assert_eq!(words[0].0, "");

    // Finalize should filter empty words
    let final_words = finalize_words(words, 1.0);
    assert!(final_words.is_empty());
}

#[test]
fn test_consecutive_space_tokens_and_space_only_tokens() {
    // Tokens consisting solely of spaces
    let toks = vec![
        (b" " as &[u8], 0.0, 0.9),
        (b" " as &[u8], 0.2, 0.8),
        (b"   " as &[u8], 0.4, 0.7),
        (b" hello" as &[u8], 1.0, 0.95),
        (b" " as &[u8], 1.5, 0.6),
        (b"world" as &[u8], 1.8, 0.85), // appends to preceding space-only token!
    ];
    let words = assemble_words(toks);
    // groups:
    // [0]: ("", 0.0, 0.9)
    // [1]: ("", 0.2, 0.8)
    // [2]: ("", 0.4, 0.7)
    // [3]: ("hello", 1.0, 0.95)
    // [4]: ("world", 1.5, min(0.6, 0.85) = 0.6)
    assert_eq!(words.len(), 5);
    assert_eq!(words[3].0, "hello");
    assert_eq!(words[4].0, "world");
    assert_eq!(words[4].1, 1.5, "Inherited start time of the space token that opened the group");
    assert_eq!(words[4].2, 0.6, "Weakest confidence of space + content");

    let final_words = finalize_words(words, 3.0);
    assert_eq!(final_words.len(), 2, "Filtered down to only non-empty words");
    assert_eq!(final_words[0].word, "hello");
    assert_eq!(final_words[1].word, "world");
}

#[test]
fn test_multiple_leading_spaces_stripped_from_word() {
    let toks = vec![
        (b"   leading" as &[u8], 1.0, 0.9),
        (b"     spaces" as &[u8], 2.0, 0.8),
    ];
    let words = assemble_words(toks);
    assert_eq!(words.len(), 2);
    assert_eq!(words[0].0, "leading");
    assert_eq!(words[1].0, "spaces");
}

#[test]
fn test_weakest_link_confidence_accumulation() {
    let toks = vec![
        (b" un" as &[u8], 1.0, 0.95),
        (b"der" as &[u8], 1.1, 0.40), // lowest confidence
        (b"stand" as &[u8], 1.2, 0.80),
        (b"ing" as &[u8], 1.3, 0.99),
    ];
    let words = assemble_words(toks);
    assert_eq!(words.len(), 1);
    assert_eq!(words[0].0, "understanding");
    assert_eq!(words[0].1, 1.0, "Starts at 1.0");
    assert_eq!(words[0].2, 0.40, "Weakest sub-token confidence wins");
}

// ── Floating Point Robustness & round_to ─────────────────────────────────────

#[test]
fn test_round_to_standard_precisions() {
    assert_eq!(round_to(1.23456, 2), 1.23);
    assert_eq!(round_to(1.23556, 2), 1.24);
    assert_eq!(round_to(1.0, 2), 1.0);
    assert_eq!(round_to(0.0, 2), 0.0);
    assert_eq!(round_to(-1.234, 2), -1.23);
    assert_eq!(round_to(-1.236, 2), -1.24);
    assert_eq!(round_to(42.0, 0), 42.0);
    assert_eq!(round_to(42.6, 0), 43.0);
    assert_eq!(round_to(125.0, -1), 130.0);
}

#[test]
fn test_round_to_extreme_floats() {
    // Zero & signed zero
    assert_eq!(round_to(0.0, 2), 0.0);
    assert_eq!(round_to(-0.0, 2), 0.0);

    // Subnormal / tiny numbers
    let tiny = 1e-308f64;
    assert_eq!(round_to(tiny, 2), 0.0);
    assert_eq!(round_to(f64::MIN_POSITIVE, 2), 0.0);

    // Large numbers
    assert_eq!(round_to(1e12 + 0.5, 0), 1e12 + 1.0);

    // Infs and NaN must not panic
    let inf = round_to(f64::INFINITY, 2);
    assert!(inf.is_infinite() && inf.is_sign_positive());

    let neg_inf = round_to(f64::NEG_INFINITY, 2);
    assert!(neg_inf.is_infinite() && neg_inf.is_sign_negative());

    let nan = round_to(f64::NAN, 2);
    assert!(nan.is_nan());
}

// ── Timestamp Monotonicity & Word Spans ───────────────────────────────────────

#[test]
fn test_timestamp_monotonicity_and_span_invariants() {
    let toks = vec![
        (b" First" as &[u8], 1.00, 0.9),
        (b" second" as &[u8], 2.50, 0.9),
        (b" third" as &[u8], 2.50, 0.9), // identical timestamp (simultaneous)
        (b" fourth" as &[u8], 4.00, 0.9),
    ];
    let groups = assemble_words(toks);
    let words = finalize_words(groups, 5.00);

    assert_eq!(words.len(), 4);

    for i in 0..words.len() {
        assert!(
            words[i].end >= words[i].start,
            "Word {} span invariant violated: start={} > end={}",
            words[i].word,
            words[i].start,
            words[i].end
        );

        if i + 1 < words.len() {
            assert!(
                words[i].end <= words[i + 1].start + 1e-6,
                "Word {} end ({}) exceeds next word start ({})",
                words[i].word,
                words[i].end,
                words[i + 1].start
            );
            assert!(
                words[i].start <= words[i + 1].start,
                "Start times must be monotonic: {} ({}) > {} ({})",
                words[i].word,
                words[i].start,
                words[i + 1].word,
                words[i + 1].start
            );
        }
    }

    assert_eq!(words.last().unwrap().end, 5.00);
}

#[test]
fn test_segment_end_before_last_word_clamped() {
    // If seg_end is mistakenly earlier than the last word's start, .max(*start) ensures end >= start
    let toks = vec![
        (b" Hello" as &[u8], 5.00, 0.9),
        (b" world" as &[u8], 6.00, 0.9),
    ];
    let groups = assemble_words(toks);
    let words = finalize_words(groups, 4.00); // seg_end is 4.00, before start of 6.00!

    assert_eq!(words[1].start, 6.00);
    assert_eq!(words[1].end, 6.00, "End must clamp to start rather than invert");
}

// ── Dialogue Dash & Turn Marker Handling ─────────────────────────────────────

#[test]
fn test_strip_turn_dash_comprehensive() {
    // 1. Single dash turn markers with and without spaces
    assert_eq!(strip_turn_dash("- Hello world"), "Hello world");
    assert_eq!(strip_turn_dash("-Hello world"), "Hello world");
    assert_eq!(strip_turn_dash("-   Lots of spaces"), "Lots of spaces");
    assert_eq!(strip_turn_dash("-\tTabbed turn"), "Tabbed turn");
    assert_eq!(strip_turn_dash("-\nNewline turn"), "Newline turn");
    assert_eq!(strip_turn_dash("-"), "");
    assert_eq!(strip_turn_dash("- "), "");

    // 2. False start markers and em-dashes (MUST BE PRESERVED)
    assert_eq!(strip_turn_dash("-- double dash"), "-- double dash");
    assert_eq!(strip_turn_dash("--- TeX em-dash"), "--- TeX em-dash");
    assert_eq!(strip_turn_dash("---- four dashes"), "---- four dashes");
    assert_eq!(strip_turn_dash("--"), "--");
    assert_eq!(strip_turn_dash("---"), "---");

    // 3. Hyphenated words & interior dashes
    assert_eq!(strip_turn_dash("die-in"), "die-in");
    assert_eq!(strip_turn_dash("Jean-Luc"), "Jean-Luc");
    assert_eq!(strip_turn_dash("A - B"), "A - B");
    assert_eq!(strip_turn_dash("it was -- in October"), "it was -- in October");

    // 4. Unicode dashes
    assert_eq!(strip_turn_dash("— unicode em-dash"), "— unicode em-dash");
    assert_eq!(strip_turn_dash("– unicode en-dash"), "– unicode en-dash");

    // 5. Empty and whitespace-only
    assert_eq!(strip_turn_dash(""), "");
    assert_eq!(strip_turn_dash("   "), "   ");
}

#[test]
fn test_dialogue_dash_dropped_from_word_list_without_corrupting_subsequent_timing() {
    let toks = vec![
        (b"-" as &[u8], 0.00, 0.9),
        (b" Why" as &[u8], 0.50, 0.95),
        (b" do" as &[u8], 0.80, 0.95),
        (b" you" as &[u8], 1.10, 0.95),
    ];
    let groups = assemble_words(toks);
    let words = finalize_words(groups, 2.00);

    // Bare "-" must be dropped
    assert_eq!(words.len(), 3);
    assert_eq!(words[0].word, "Why");
    assert_eq!(words[0].start, 0.50, "First word starts at 0.50, not 0.00");
    assert_eq!(words[0].end, 0.80);
    assert_eq!(words[1].word, "do");
    assert_eq!(words[2].word, "you");
    assert_eq!(words[2].end, 2.00);
}

#[test]
fn test_double_dash_preserved_in_word_list() {
    let toks = vec![
        (b"--" as &[u8], 0.00, 0.9),
        (b" it" as &[u8], 0.50, 0.95),
        (b" would" as &[u8], 0.80, 0.95),
    ];
    let groups = assemble_words(toks);
    let words = finalize_words(groups, 2.00);

    // "--" must NOT be dropped
    assert_eq!(words.len(), 3);
    assert_eq!(words[0].word, "--");
    assert_eq!(words[0].start, 0.00);
    assert_eq!(words[0].end, 0.50);
}

#[test]
fn test_utf8_1byte_split_torture() {
    // Break an entire multi-byte phrase into 1-byte chunks without spaces
    // to verify that word assembly reassembles the full string without corruption.
    let text = "Übergrößenträger";
    let bytes = text.as_bytes();

    let mut toks: Vec<(&[u8], f64, f32)> = Vec::new();
    let mut t = 0.0;
    for i in 0..bytes.len() {
        let slice = &bytes[i..i + 1];
        toks.push((slice, t, 0.95));
        t += 0.01;
    }

    let words = assemble_words(toks);
    assert_eq!(words.len(), 1, "Must assemble into exactly 1 word");
    assert_eq!(words[0].0, text, "Must match original UTF-8 text perfectly");
    assert!(!words[0].0.contains('\u{FFFD}'), "Zero corruption characters");
    assert_eq!(words[0].1, 0.0, "Starts at 0.0");
}

#[test]
fn test_interior_hyphen_never_dropped() {
    // If a hyphen appears mid-sentence (index >= 1), it must NOT be dropped as a turn marker
    let toks = vec![
        (b" item" as &[u8], 1.0, 0.9),
        (b" -" as &[u8], 1.5, 0.9),
        (b" one" as &[u8], 2.0, 0.9),
    ];
    let groups = assemble_words(toks);
    let words = finalize_words(groups, 3.0);

    assert_eq!(words.len(), 3, "Interior hyphen must survive");
    assert_eq!(words[0].word, "item");
    assert_eq!(words[1].word, "-");
    assert_eq!(words[2].word, "one");
}

#[test]
fn test_single_word_segment() {
    let toks = vec![(b" Solitary" as &[u8], 2.50, 0.99)];
    let groups = assemble_words(toks);
    let words = finalize_words(groups, 5.00);

    assert_eq!(words.len(), 1);
    assert_eq!(words[0].word, "Solitary");
    assert_eq!(words[0].start, 2.50);
    assert_eq!(words[0].end, 5.00, "Single word takes segment end");
    assert_eq!(words[0].p, 0.99);
}

#[test]
fn test_extreme_probabilities_and_negative_timestamps() {
    let toks = vec![
        (b" negative" as &[u8], -5.00, 1.0),
        (b" zero" as &[u8], 0.00, 0.0),
        (b" subnormal" as &[u8], 1.00, 1e-40f32),
        (b" nan" as &[u8], 2.00, f32::NAN),
    ];
    let groups = assemble_words(toks);
    let words = finalize_words(groups, 3.00);

    assert_eq!(words.len(), 4);
    assert_eq!(words[0].start, -5.00);
    assert_eq!(words[0].p, 1.0);
    assert_eq!(words[1].p, 0.0);
    assert_eq!(words[2].p, 0.0); // round_to(1e-40, 2) is 0.0
    // NaN handling in round_to produces NaN, but doesn't panic
    assert!(words[3].p.is_nan());
}

