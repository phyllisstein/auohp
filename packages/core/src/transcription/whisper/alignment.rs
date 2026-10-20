//! Dynamic Time Warping (DTW) word assembly, timestamp math, and byte-level BPE safety.
//!
//! whisper.cpp emits discrete centisecond instants (`t_dtw`) via DTW on cross-attention
//! heads. Words are assembled by grouping BPE tokens by space prefix. Because multi-byte
//! UTF-8 code points can be split across BPE tokens, decoding is deferred to word
//! boundaries to prevent UTF-8 corruption.

use anyhow::{Context as _, Result};

use super::super::types::Word;

/// Round `x` to `places` decimal places (nearest, not truncated, to avoid a
/// systematic downward bias).
pub fn round_to(x: f64, places: i32) -> f64 {
    let factor = 10f64.powi(places);
    (x * factor).round() / factor
}

/// Drop a leading dialogue dash.
///
/// Whisper sometimes opens a segment with `- ` to mark a speaker change, which
/// this pipeline cannot honour and which renders as a stray hyphen. Only a single
/// hyphen at the very start is stripped: mid-segment `--` is a genuine
/// false-start marker (`"it would be around -- it would"`) and must survive.
/// Across 8300 segments, every leading dash was a bare `-` and every `--` sat
/// mid-segment.
pub fn strip_turn_dash(text: &str) -> &str {
    match text.strip_prefix('-') {
        // `--` and longer runs are false-start markers; leave them be.
        Some(rest) if !rest.starts_with('-') => rest.trim_start(),
        _ => text,
    }
}

/// Group a token stream into words, decoding UTF-8 once per word.
///
/// Whisper's vocabulary is byte-level BPE, so a multi-byte character can arrive
/// split across tokens, neither valid UTF-8 alone. Only word boundaries are
/// guaranteed character-whole, so decoding happens there; per-token lossy
/// decoding would turn a split 3-byte character into two U+FFFD. A leading
/// `0x20` is always a real separator, since UTF-8 continuation bytes have the
/// high bit set.
pub fn assemble_words<'a, I>(toks: I) -> Vec<(String, f64, f32)>
where
    I: IntoIterator<Item = (&'a [u8], f64, f32)>,
{
    let mut groups: Vec<(Vec<u8>, f64, f32)> = Vec::new();
    for (bytes, at, p) in toks {
        // The first token opens a word even without a leading space.
        if bytes.first() == Some(&b' ') || groups.is_empty() {
            let start = bytes.iter().take_while(|b| **b == b' ').count();
            groups.push((bytes[start..].to_vec(), at, p));
        } else {
            let last = groups.last_mut().expect("non-empty by construction");
            last.0.extend_from_slice(bytes);
            // A word is only as confident as its weakest sub-token.
            last.2 = last.2.min(p);
        }
    }
    groups
        .into_iter()
        .map(|(b, at, p)| (String::from_utf8_lossy(&b).into_owned(), at, p))
        .collect()
}

/// Group per-token DTW timing from a single segment into words.
///
/// A token whose text begins with an ASCII space starts a new word. Special
/// tokens are filtered by id (`id >= special_min`) rather than by text: text
/// matching let `[_TT_n]`, which has no leading space, concatenate onto the
/// preceding word and corrupt the search index.
///
/// `t_dtw` is an instant, not a span, so a word ends where the next begins and
/// the last word takes the segment's end.
pub fn collect_words(
    seg: &whisper_rs::WhisperSegment<'_>,
    seg_start: f64,
    seg_end: f64,
    special_min: i32,
) -> Result<Vec<Word>> {
    /// One content token. Bytes rather than `&str`: a multi-byte character can
    /// be split across tokens, and `to_str()` errors on such a fragment.
    struct Tok<'a> {
        bytes: &'a [u8],
        at: f64,
        p: f32,
    }

    let n_tokens = seg.n_tokens();
    let mut toks: Vec<Tok<'_>> = Vec::with_capacity(n_tokens as usize);

    // whisper.cpp initialises `t_dtw` to -1 and assigns it only where the DTW
    // backtrace changes column, so unset times are normal. Carrying the last
    // known instant forward keeps the stream monotonic; clamping to 0.0 would
    // drag a mid-segment word to the recording's start.
    let mut last_at = seg_start;

    for j in 0..n_tokens {
        let token = seg.get_token(j).context("token index out of bounds")?;

        // Times are i64 centiseconds.
        let td = token.token_data();
        if td.id >= special_min {
            continue;
        }

        let bytes = token.to_bytes().context("failed to read token bytes")?;

        let at = if td.t_dtw >= 0 {
            td.t_dtw as f64 / 100.0
        } else if td.t0 >= 0 {
            td.t0 as f64 / 100.0
        } else {
            last_at
        };
        let at = round_to(at, 2).max(last_at);
        last_at = at;

        toks.push(Tok {
            bytes,
            at,
            p: td.p,
        });
    }

    let mut groups = assemble_words(toks.iter().map(|t| (t.bytes, t.at, t.p)));

    // Whisper emits the dialogue dash as its own word at index 0; see
    // `strip_turn_dash`. Dropping it leaves timing intact, since each word ends at
    // the next word's start.
    if groups.first().is_some_and(|(w, _, _)| w == "-") {
        groups.remove(0);
    }

    // A word ends where the next begins; the last one ends with the segment.
    let words = groups
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
        .collect();

    Ok(words)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every leading dash observed across 8300 segments was this shape.
    #[test]
    fn a_leading_dialogue_dash_is_markup_and_goes() {
        assert_eq!(
            strip_turn_dash("- Why do you start with the hard questions?"),
            "Why do you start with the hard questions?"
        );
        assert_eq!(strip_turn_dash("-No space either"), "No space either");
    }

    /// Mid-segment `--` is a false-start marker and must survive.
    #[test]
    fn false_start_markers_survive() {
        for s in [
            "about the CDC would be around -- it would",
            "time. So it was -- in October, we did the one at HHS",
            "And we did a lot of -- that was the first time",
        ] {
            assert_eq!(strip_turn_dash(s), s, "interior -- must be preserved");
        }
    }

    /// A segment *opening* with `--` is a false start, not a turn marker.
    #[test]
    fn a_leading_double_dash_is_not_a_turn_marker() {
        assert_eq!(
            strip_turn_dash("-- it would take place"),
            "-- it would take place"
        );
        assert_eq!(strip_turn_dash("---"), "---");
    }

    /// Hyphenated words are untouched.
    #[test]
    fn hyphenated_words_are_untouched() {
        assert_eq!(strip_turn_dash("die-in at the FDA"), "die-in at the FDA");
        assert_eq!(strip_turn_dash("Sloan-Kettering"), "Sloan-Kettering");
    }

    /// A right single quote (U+2019, `e2 80 99`) arriving as two BPE tokens,
    /// neither valid UTF-8 alone.
    #[test]
    fn a_character_split_across_tokens_reassembles() {
        let words = assemble_words(vec![
            (b" don" as &[u8], 1.0, 0.9),
            (&[0xe2, 0x80], 1.1, 0.8), // first two bytes of U+2019
            (&[0x99], 1.2, 0.7),       // ...and the third
            (b"t" as &[u8], 1.3, 0.95),
        ]);
        assert_eq!(words.len(), 1, "one word, not four");
        assert_eq!(words[0].0, "don\u{2019}t");
        assert!(!words[0].0.contains('\u{fffd}'), "no replacement chars");
        assert_eq!(words[0].1, 1.0, "the word starts where its first token did");
        assert_eq!(words[0].2, 0.7, "weakest sub-token wins");
    }

    /// Per-token lossy decoding of the fragments above would yield two U+FFFD.
    #[test]
    fn per_token_decoding_would_have_corrupted_this() {
        let fragments: Vec<u8> = [0xe2u8, 0x80, 0x99].to_vec();
        let per_token: String = [&fragments[..2], &fragments[2..]]
            .iter()
            .map(|b| String::from_utf8_lossy(b).into_owned())
            .collect();
        assert_eq!(per_token, "\u{fffd}\u{fffd}", "the trap this fix avoids");
        assert_eq!(String::from_utf8_lossy(&fragments), "\u{2019}");
    }

    /// Multi-byte text that is *not* split must survive untouched, and interior
    /// bytes must never be mistaken for the leading-space separator.
    #[test]
    fn whole_multibyte_tokens_pass_through() {
        let words = assemble_words(vec![
            (" café".as_bytes(), 0.0, 0.9),
            (" —".as_bytes(), 0.5, 0.8),
        ]);
        assert_eq!(
            words.iter().map(|w| w.0.as_str()).collect::<Vec<_>>(),
            vec!["café", "—"]
        );
    }

    /// A segment need not begin on a word boundary, so the first token opens a
    /// word even with no leading space.
    #[test]
    fn a_segment_may_open_mid_word() {
        let words = assemble_words(vec![
            (b"ing" as &[u8], 0.0, 0.9),
            (b" up" as &[u8], 0.4, 0.9),
        ]);
        assert_eq!(
            words.iter().map(|w| w.0.as_str()).collect::<Vec<_>>(),
            vec!["ing", "up"]
        );
    }
}
