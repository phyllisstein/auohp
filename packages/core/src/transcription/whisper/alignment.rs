//! Dynamic Time Warping (DTW) word assembly, timestamp math, and byte-level BPE safety.
//!
//! whisper.cpp emits discrete centisecond instants (`t_dtw`) via DTW on cross-attention
//! heads. Words are assembled by grouping BPE tokens by space prefix. Because multi-byte
//! UTF-8 code points can be split across BPE tokens, decoding is deferred to word
//! boundaries to prevent UTF-8 corruption.

use anyhow::{Context as _, Result};

use super::super::types::Word;

/// Round `x` to `places` decimal places.
///
/// We *round* (nearest) rather than truncate (toward zero): truncation would
/// bias every value downward---confidences always a hair low, start times
/// always a hair early---and that bias compounds across thousands of words.
/// Whisper's timing grid is centiseconds and `p` is a coarse editorial signal,
/// so two decimals is the real information content; more is invented precision.
pub fn round_to(x: f64, places: i32) -> f64 {
    let factor = 10f64.powi(places);
    (x * factor).round() / factor
}

/// Drop a leading dialogue dash---markup Whisper invents, not speech.
///
/// Whisper sometimes opens a segment with `- ` to mark a change of speaker, having
/// learned the screenplay/subtitle convention from its training data. It is a
/// *turn indicator*, and a false one: this pipeline assigns no speakers, so the
/// dash asserts a structure nothing downstream can honour, and it renders in a
/// caption as a stray hyphen.
///
/// Same class of thing as the `[_BEG_]` control tokens---text the model emits
/// about the transcript rather than words anyone said---so removing it is not
/// the editorial post-processing that was ruled out.
///
/// **Deliberately narrow.** Only a *single* hyphen, and only at the very start.
/// Mid-segment `--` is a genuine false-start marker and the most readable thing in
/// the output: `"it would be around -- it would"`, `"So it was -- in October"`.
/// Measured over 8300 segments of interviews 043, 047, 108 and the Ashes footage:
/// every leading dash was a bare `-` at word 0, and every `--` sat mid-segment
/// (indices 15, 4, 6). The two never overlap, so position plus length separates
/// them exactly.
pub fn strip_turn_dash(text: &str) -> &str {
    match text.strip_prefix('-') {
        // `--` and longer runs are false-start markers; leave them be.
        Some(rest) if !rest.starts_with('-') => rest.trim_start(),
        _ => text,
    }
}

/// Group a token stream into words, decoding UTF-8 once per word.
///
/// Split out from `collect_words` so it can be tested without a live Whisper
/// state, because the interesting case is invisible from the outside: whisper's
/// vocabulary is byte-level BPE, so a multi-byte character may arrive as two
/// tokens neither of which is valid UTF-8 alone.
///
/// The word boundary is the *only* boundary where a character is guaranteed
/// whole, so it is the only place decoding may happen. Decoding per token---which
/// is what `to_str_lossy()` on each token would do, and which looks correct---turns
/// a 3-byte character split 2/1 into two U+FFFD replacements instead of one
/// character. `from_utf8_lossy` here fires only if a *word* ends mid-character.
///
/// A leading `0x20` is a real separator and never a fragment: every UTF-8
/// continuation byte has its high bit set, so a space cannot occur inside a
/// multi-byte sequence.
pub fn assemble_words<'a, I>(toks: I) -> Vec<(String, f64, f32)>
where
    I: IntoIterator<Item = (&'a [u8], f64, f32)>,
{
    let mut groups: Vec<(Vec<u8>, f64, f32)> = Vec::new();
    for (bytes, at, p) in toks {
        // The first content token opens a word even without a leading space,
        // since a segment need not begin on a word boundary.
        if bytes.first() == Some(&b' ') || groups.is_empty() {
            let start = bytes.iter().take_while(|b| **b == b' ').count();
            groups.push((bytes[start..].to_vec(), at, p));
        } else {
            let last = groups.last_mut().expect("non-empty by construction");
            last.0.extend_from_slice(bytes);
            // Weakest link wins: a word is only as trustworthy as its least
            // confident sub-token.
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
/// whisper.cpp uses the BPE space-prefix convention: a token whose decoded text
/// begins with an ASCII space marks the start of a new word.
///
/// Two things here are load-bearing:
///
/// **Special tokens are filtered by id, not by text.** The vocabulary reserves a
/// block at the top for them, so `id >= special_min` catches every form. Matching
/// on text is what let `[_BEG_]` and `[_TT_n]` through previously---and `[_TT_n]`
/// carries no leading space, so it was silently concatenated onto the preceding
/// word (`"you.[_TT_1499]"`), corrupting the text that reaches the search index.
///
/// **`t_dtw` is an instant, not a span.** A word therefore has no end time of its
/// own; it ends where the next word begins. That is why this builds the token
/// list first and then zips over adjacent pairs, rather than folding intervals in
/// a single pass. The last word is the only one with nothing to zip against, and
/// takes the segment's end.
pub fn collect_words(
    seg: &whisper_rs::WhisperSegment<'_>,
    seg_start: f64,
    seg_end: f64,
    special_min: i32,
) -> Result<Vec<Word>> {
    /// One content token: its **bytes**, the instant DTW placed it at, and its
    /// probability.
    ///
    /// Bytes, not `&str`, because whisper's vocabulary is byte-level BPE: a
    /// multi-byte character can be split across two tokens, and neither fragment
    /// is valid UTF-8 on its own. `to_str()` returns `Err(InvalidUtf8)` for such a
    /// token, which killed run `033-full-108` after 60 minutes of GPU time---the
    /// inference had already produced all 2739 segments.
    struct Tok<'a> {
        bytes: &'a [u8],
        at: f64,
        p: f32,
    }

    // n_tokens is a bare c_int---no Result.
    let n_tokens = seg.n_tokens();
    let mut toks: Vec<Tok<'_>> = Vec::with_capacity(n_tokens as usize);

    // whisper.cpp initialises every token to `{..., t0: -1, t1: -1, t_dtw: -1, ...}`
    // and then assigns `t_dtw` only where the DTW backtrace changes column. Tokens
    // it never reaches keep the sentinel, so an unset time is normal rather than
    // exceptional and has to degrade gracefully.
    //
    // Carrying the last known instant forward is what keeps the stream monotonic.
    // Substituting 0.0 (which is what clamping a negative would do) would drag a
    // mid-segment word back to the start of the recording---far worse than a
    // word that merely shares a timestamp with its predecessor.
    let mut last_at = seg_start;

    for j in 0..n_tokens {
        // get_token returns Option<WhisperToken<'_, '_>>; bounds are guaranteed here.
        let token = seg.get_token(j).context("token index out of bounds")?;

        // token_data() returns WhisperTokenData (whisper_rs_sys::whisper_token_data)
        // directly---not a Result. Times are i64 centiseconds.
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

    // The dialogue dash arrives as its own word, always at index 0. Dropping it
    // costs no timing: `collect_words` gives every word an end equal to the *next*
    // word's start, so the following word's span is untouched by the removal.
    // See `strip_turn_dash` for why this is markup rather than speech, and why the
    // test is a bare `-` and not any hyphen.
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

    /// The case this must never touch. Mid-segment `--` is a false-start marker and
    /// the most readable thing in the output; a rule that swallowed it would be a
    /// regression dressed as a cleanup.
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

    /// A segment *opening* with `--` is a false start carried across a boundary, not
    /// a turn marker. Length is what separates the two, so pin it.
    #[test]
    fn a_leading_double_dash_is_not_a_turn_marker() {
        assert_eq!(
            strip_turn_dash("-- it would take place"),
            "-- it would take place"
        );
        assert_eq!(strip_turn_dash("---"), "---");
    }

    /// Hyphenated words must be untouched---`strip_prefix` only ever fires at
    /// position zero, but this is the reading that would break if it did not.
    #[test]
    fn hyphenated_words_are_untouched() {
        assert_eq!(strip_turn_dash("die-in at the FDA"), "die-in at the FDA");
        assert_eq!(strip_turn_dash("Sloan-Kettering"), "Sloan-Kettering");
    }

    /// The failure that cost 60 minutes of completed inference on run
    /// `033-full-108`: a right single quote (U+2019, `e2 80 99`) arriving as two
    /// BPE tokens, neither valid UTF-8 alone.
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

    /// Decoding per token instead of per word is the plausible-looking wrong fix.
    /// This pins the distinction: lossy applied to each fragment above would give
    /// two U+FFFD, so a test that only checked "does not error" would pass while
    /// the text was corrupted.
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
