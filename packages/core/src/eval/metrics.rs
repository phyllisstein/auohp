//! Scorecard construction: WER, lexicon recall, timing drift, structural stats.

use serde::{Deserialize, Serialize};

use super::align::{align, anchor, Op};
use super::normalize::{is_filler, normalize, FILLER_PHRASES};
use crate::transcription::TranscriptionResult;

/// Bumped whenever scoring behaviour changes.
///
/// Scores are only meaningful relative to one another, so a metric change
/// silently makes archived runs incomparable to new ones. That failure is
/// dangerous precisely because it does not error --- it just quietly corrupts
/// every future comparison. On every bump, re-score the archived `result.json`
/// files (CPU-only, no GPU time) and mark the boundary in the tuning ledger.
/// - v2: symbols spoken as words (`=`, `&`, `%`, `+`) normalise to those words
///   rather than being stripped, and lexicon entries that normalise alike are
///   collapsed.
/// - v4: `anchor` handles partial coverage. When a transcript covers more tape
///   than the clip drawn from it, the truth's tail has no counterpart, its vote
///   is noise, and believing it collapsed the window to a sixth of the real
///   overlap. Adds `structure.speaker` and `partial_coverage`.
/// - v3: contraction clitics that lost their apostrophe are reattached. Whisper
///   intermittently emits `wouldn t` / `I m`, which previously became a spurious
///   substitution *and* insertion apiece. This alone cut one run's substitution
///   count by a quarter, so v2 substitution counts are not comparable to v3.
pub const HARNESS_VERSION: u32 = 4;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scorecard {
    pub harness_version: u32,
    pub wer: WordErrorRate,
    pub lexicon: LexiconReport,
    pub drift: Vec<AnchorDrift>,
    pub structure: StructureStats,
    /// Diagonal-vote support for the truth/hypothesis alignment. Near zero means
    /// the two streams may not be the same recording, and every other number on
    /// this card should be disregarded rather than interpreted.
    pub anchor_confidence: usize,
    pub anchored_truth_range: (usize, usize),
    pub anchored_hyp_range: (usize, usize),
    /// The transcript and the clip cover different extents, so only their overlap
    /// was scored. Every figure below describes that overlap, not the whole file.
    pub partial_coverage: bool,
    pub taxonomy: ErrorTaxonomy,
}

/// The most frequent errors, ranked. Carried on the scorecard so the analyst can
/// work from `score.json` without re-running the alignment.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ErrorTaxonomy {
    /// `(truth, hyp, count)`, most frequent first.
    pub substitutions: Vec<(String, String, usize)>,
    /// `(token, count)` for words the hypothesis added.
    pub insertions: Vec<(String, usize)>,
    /// `(token, count)` for words the hypothesis dropped.
    pub deletions: Vec<(String, usize)>,
}

/// Word error rate and its parts.
///
/// **`rate` is not a grade.** The ground truth is OCR'd and editorially cleaned,
/// with disfluencies silently removed, so a flawless transcription still scores
/// poorly against it. This number is only ever a *relative* signal between
/// configs on the same fixture.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WordErrorRate {
    pub rate: f64,
    pub subs: usize,
    pub dels: usize,
    /// Insertions whose token is a filler ("um", "you know"). Almost always the
    /// human transcriber's editorial habit rather than a model defect.
    pub ins_filler: usize,
    /// Insertions of substantive words. This is the bucket that can indicate
    /// genuine hallucination, and the only insertion count worth reacting to.
    pub ins_content: usize,
    pub truth_tokens: usize,
    pub hyp_tokens: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LexiconReport {
    /// Recall over terms that actually occur in this truth. Terms absent from the
    /// truth are excluded, so a corpus-wide lexicon costs nothing when scoring a
    /// single interview.
    pub recall: f64,
    pub terms: Vec<LexiconRecall>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LexiconRecall {
    pub term: String,
    pub expected: usize,
    pub found: usize,
    /// What the model produced instead, gathered from the aligned substitutions
    /// that overlap this term's positions. This is the actionable part: it turns
    /// "recall is 0.6" into "it writes `acta` for `act up`".
    pub confusions: Vec<String>,
}

/// Observed media time for a tape marker, and the fit that maps between them.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnchorDrift {
    pub label: String,
    pub tape: u32,
    pub tape_seconds: f64,
    /// `None` when the anchor phrase could not be located in the hypothesis.
    pub media_seconds: Option<f64>,
    /// Residual against the per-tape linear fit, in seconds.
    pub residual: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructureStats {
    pub segments: usize,
    pub words: usize,
    /// Whisper control tokens that leaked into word text (`[_BEG_]`, `[_TT_42]`).
    /// Must be zero. Non-zero means corrupted text is reaching the search index.
    pub control_token_words: usize,
    pub zero_duration_words: usize,
    /// Words whose span implies an implausible speech rate --- a symptom of
    /// timestamps being smeared across a VAD window rather than DTW-aligned.
    pub implausible_duration_words: usize,
    pub backwards_time_words: usize,
    /// Fraction of segment starts landing within 100 ms of a 30 s multiple.
    /// High values mean boundaries are following VAD windows, not speech.
    pub boundary_quantization: f64,
    pub median_segment_seconds: f64,
    /// How well segment boundaries respect speaker turns. `None` when no turns
    /// fixture was supplied.
    pub speaker: Option<SpeakerBoundaries>,
    /// Per-tape linear fit of media time against tape time. Slope should be 1.0;
    /// a slope materially off 1.0 means decoded audio runs fast or slow, which
    /// indicts the resampler rather than the decoder.
    pub time_slope: Option<f64>,
}

/// Whether VAD segment boundaries happen to fall on speaker changes.
///
/// The pipeline does not diarize --- speaker labels are assigned by hand later.
/// But if segmentation already breaks where the speaker changes, the remaining
/// work is *labelling existing segments* (a two-class assignment) rather than
/// *detecting boundaries*, which is a far easier problem and the one that got
/// diarization abandoned in the first place. So this is worth measuring even
/// though nothing currently consumes it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeakerBoundaries {
    /// Speaker changes that could be located in the hypothesis.
    pub changes: usize,
    /// …of which a segment boundary falls in the gap between the two turns.
    pub covered: usize,
    /// …and of which none does, so one segment carries both speakers.
    pub bleed: usize,
    /// How many `covered` would be expected if boundaries fell at random with
    /// this segment density. **Read `lift`, not `covered`** --- a config that
    /// simply cuts more often scores higher on `covered` for no real reason.
    pub expected_by_chance: f64,
    /// `covered / expected_by_chance`. 1.0 means the segmentation knows nothing
    /// about speakers.
    pub lift: f64,
    pub mean_segment_tokens: f64,
}

/// One speaker turn from the truth transcript.
#[derive(Debug, Clone, Deserialize)]
pub struct Turn {
    pub speaker: String,
    pub text: String,
}

#[derive(Debug, Deserialize)]
pub struct TurnsFile {
    pub turns: Vec<Turn>,
}

/// A domain term, stored both as written and as its normalised token sequence.
pub struct Lexicon {
    pub terms: Vec<(String, Vec<String>)>,
}

impl Lexicon {
    /// Parse the lexicon fixture: one term per line, `#` comments ignored.
    ///
    /// Entries that normalise to the same token sequence are collapsed, keeping
    /// the first spelling. `Silence = Death` and `SILENCE = DEATH` are one term
    /// once cased and punctuation are folded away; counting them twice would
    /// inflate both the numerator and denominator of recall, and silently weight
    /// whichever terms happened to be written more than one way.
    pub fn parse(src: &str) -> Self {
        let mut seen: std::collections::HashSet<Vec<String>> = Default::default();
        let terms = src
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .filter_map(|l| {
                let toks = normalize(l);
                (!toks.is_empty() && seen.insert(toks.clone())).then(|| (l.to_string(), toks))
            })
            .collect();
        Lexicon { terms }
    }
}

/// Count non-overlapping occurrences of `needle` in `hay`, returning start indices.
fn find_all(hay: &[String], needle: &[String]) -> Vec<usize> {
    if needle.is_empty() || hay.len() < needle.len() {
        return vec![];
    }
    let mut hits = Vec::new();
    let mut i = 0;
    while i + needle.len() <= hay.len() {
        if hay[i..i + needle.len()] == *needle {
            hits.push(i);
            i += needle.len();
        } else {
            i += 1;
        }
    }
    hits
}

/// Score a hypothesis against the truth.
pub fn score(
    truth_text: &str,
    hyp: &TranscriptionResult,
    lex: &Lexicon,
    anchors: &[AnchorSpec],
) -> Scorecard {
    score_with_turns(truth_text, hyp, lex, anchors, &[])
}

/// Score, additionally measuring segment boundaries against speaker turns.
pub fn score_with_turns(
    truth_text: &str,
    hyp: &TranscriptionResult,
    lex: &Lexicon,
    anchors: &[AnchorSpec],
    turns: &[Turn],
) -> Scorecard {
    let truth_all = normalize(truth_text);
    let hyp_all =
        normalize(&hyp.segments.iter().map(|s| s.text.as_str()).collect::<Vec<_>>().join(" "));

    // Clamp both streams to the span they share. The hypothesis may cover a whole
    // interview of which the truth is only a middle excerpt, so the hypothesis
    // side has to be cut too --- otherwise its surplus becomes tens of thousands
    // of phantom insertions.
    let a = anchor(&truth_all, &hyp_all);
    let truth = &truth_all[a.truth.clone()];
    let hyp_tokens = hyp_all[a.hyp.clone()].to_vec();

    let ops = align(truth, &hyp_tokens);

    let subs = ops.iter().filter(|o| matches!(o, Op::Sub { .. })).count();
    let dels = ops.iter().filter(|o| matches!(o, Op::Del(_))).count();
    let (ins_filler, ins_content) = classify_insertions(&ops);

    // Conventional WER counts every insertion. Fillers are reported separately
    // above so the number can be read with the transcriber's habits in mind, but
    // they are not silently discounted here --- that would make this rate
    // incomparable to any WER anyone else computes.
    let errors = subs + dels + ins_filler + ins_content;
    let wer = WordErrorRate {
        rate: if truth.is_empty() { 0.0 } else { errors as f64 / truth.len() as f64 },
        subs,
        dels,
        ins_filler,
        ins_content,
        truth_tokens: truth.len(),
        hyp_tokens: hyp_tokens.len(),
    };

    let lexicon = score_lexicon(truth, &hyp_tokens, &ops, lex);
    let (drift, time_slope) = score_drift(hyp, &truth_all, anchors);
    let speaker = score_speaker_boundaries(hyp, &truth_all, turns);
    let mut structure = structure_stats(hyp, time_slope);
    structure.speaker = speaker;

    Scorecard {
        harness_version: HARNESS_VERSION,
        wer,
        lexicon,
        drift,
        structure,
        anchor_confidence: a.confidence,
        anchored_truth_range: (a.truth.start, a.truth.end),
        anchored_hyp_range: (a.hyp.start, a.hyp.end),
        partial_coverage: a.partial_coverage,
        taxonomy: taxonomy(&ops),
    }
}

/// Rank the edit script by frequency.
fn taxonomy(ops: &[Op]) -> ErrorTaxonomy {
    use std::collections::HashMap;
    let (mut subs, mut ins, mut dels) = (
        HashMap::<(&str, &str), usize>::new(),
        HashMap::<&str, usize>::new(),
        HashMap::<&str, usize>::new(),
    );
    for op in ops {
        match op {
            Op::Sub { truth, hyp } => *subs.entry((truth, hyp)).or_default() += 1,
            Op::Ins(t) => *ins.entry(t).or_default() += 1,
            Op::Del(t) => *dels.entry(t).or_default() += 1,
            Op::Match(_) => {}
        }
    }
    fn rank1(m: HashMap<&str, usize>, n: usize) -> Vec<(String, usize)> {
        let mut v: Vec<_> = m.into_iter().map(|(t, c)| (t.to_string(), c)).collect();
        v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        v.truncate(n);
        v
    }
    let mut s: Vec<_> = subs
        .into_iter()
        .map(|((t, h), c)| (t.to_string(), h.to_string(), c))
        .collect();
    s.sort_by(|a, b| b.2.cmp(&a.2).then(a.0.cmp(&b.0)));
    s.truncate(40);

    ErrorTaxonomy { substitutions: s, insertions: rank1(ins, 40), deletions: rank1(dels, 40) }
}

/// Split insertions into "the transcriber tidied this away" and "the model may
/// have invented this".
///
/// Only the second is evidence of a defect, and conflating them makes a clean
/// transcription of a disfluent speaker look like a hallucinating model. Three
/// signatures mark an insertion as editorial rather than invented:
///
/// 1. **Interjections** --- "um", "uh".
/// 2. **Verbal tics**, matched as whole phrases against a run of consecutive
///    insertions. "you know" is a tic; `you` and `know` are ordinary words, so
///    the phrase is the only safe unit.
/// 3. **Repetitions** --- an inserted token identical to the matched token beside
///    it, which is a stutter or false start the transcriber wrote once.
fn classify_insertions(ops: &[Op]) -> (usize, usize) {
    let (mut filler, mut content) = (0usize, 0usize);
    let mut i = 0;

    while i < ops.len() {
        let Op::Ins(_) = &ops[i] else {
            i += 1;
            continue;
        };

        // Gather the whole run of consecutive insertions; a tic spans several.
        let start = i;
        while matches!(ops.get(i), Some(Op::Ins(_))) {
            i += 1;
        }
        let run: Vec<&str> = ops[start..i]
            .iter()
            .map(|o| match o {
                Op::Ins(t) => t.as_str(),
                _ => unreachable!("run is all insertions by construction"),
            })
            .collect();

        // Neighbouring matched tokens, for spotting stutters.
        let before = match ops[..start].iter().rev().find(|o| !matches!(o, Op::Ins(_))) {
            Some(Op::Match(t)) => Some(t.as_str()),
            _ => None,
        };
        let after = match ops.get(i) {
            Some(Op::Match(t)) => Some(t.as_str()),
            _ => None,
        };

        let mut k = 0;
        while k < run.len() {
            // Longest phrase match first, so "you know" beats any single-word rule.
            let phrase = FILLER_PHRASES
                .iter()
                .filter(|p| run.len() - k >= p.len() && run[k..k + p.len()] == ***p)
                .max_by_key(|p| p.len());

            if let Some(p) = phrase {
                filler += p.len();
                k += p.len();
            } else {
                let tok = run[k];
                let repeats = Some(tok) == before || Some(tok) == after;
                if is_filler(tok) || repeats {
                    filler += 1;
                } else {
                    content += 1;
                }
                k += 1;
            }
        }
    }

    (filler, content)
}

fn score_lexicon(
    truth: &[String],
    hyp: &[String],
    ops: &[Op],
    lex: &Lexicon,
) -> LexiconReport {
    // Substitutions indexed by the truth token they replaced, so a term that went
    // missing can report what the model wrote instead.
    let mut confusion_by_token: std::collections::HashMap<&str, Vec<&str>> = Default::default();
    for op in ops {
        if let Op::Sub { truth: t, hyp: h } = op {
            confusion_by_token.entry(t.as_str()).or_default().push(h.as_str());
        }
    }

    let mut terms = Vec::new();
    let (mut tot_expected, mut tot_found) = (0usize, 0usize);

    for (display, toks) in &lex.terms {
        let expected = find_all(truth, toks).len();
        if expected == 0 {
            continue; // not in this interview; a corpus-wide entry costs nothing
        }
        let found = find_all(hyp, toks).len().min(expected);

        let mut confusions: Vec<String> = Vec::new();
        if found < expected {
            for t in toks {
                if let Some(cs) = confusion_by_token.get(t.as_str()) {
                    confusions.extend(cs.iter().map(|s| s.to_string()));
                }
            }
            confusions.sort();
            confusions.dedup();
            confusions.truncate(6);
        }

        tot_expected += expected;
        tot_found += found;
        terms.push(LexiconRecall { term: display.clone(), expected, found, confusions });
    }

    terms.sort_by(|a, b| {
        let miss = |r: &LexiconRecall| r.expected - r.found;
        miss(b).cmp(&miss(a)).then(b.expected.cmp(&a.expected))
    });

    LexiconReport {
        recall: if tot_expected == 0 { 1.0 } else { tot_found as f64 / tot_expected as f64 },
        terms,
    }
}

/// A tape marker from the anchors fixture.
#[derive(Debug, Clone, Deserialize)]
pub struct AnchorSpec {
    pub tape: u32,
    pub label: String,
    pub tape_seconds: f64,
    pub following_text: String,
}

#[derive(Debug, Deserialize)]
pub struct AnchorsFile {
    pub anchors: Vec<AnchorSpec>,
}

/// Locate each anchor phrase in the timed word stream and fit media time against
/// tape time.
///
/// The fixture does not start at a tape boundary, so the offset is unknown and
/// must be fit rather than assumed. The *slope* is the part worth watching: it
/// should come out at 1.0, and a slope materially off 1.0 means decoded audio
/// runs fast or slow. That would otherwise surface only as a diffuse WER penalty
/// with no obvious cause.
fn score_drift(
    hyp: &TranscriptionResult,
    truth_all: &[String],
    anchors: &[AnchorSpec],
) -> (Vec<AnchorDrift>, Option<f64>) {
    // Flatten to a timed word stream, normalised the same way as everything else
    // so anchor phrases match on the same terms.
    let mut words: Vec<(String, f64)> = Vec::new();
    for seg in &hyp.segments {
        for w in &seg.words {
            for t in normalize(&w.word) {
                words.push((t, w.start));
            }
        }
    }
    let full: Vec<String> = words.iter().map(|(t, _)| t.clone()).collect();

    // Restrict the search to the span the truth actually covers.
    //
    // This stream is tokenised from `words[]` while the WER stream comes from
    // segment `text`, so their indices do not correspond and the WER anchoring
    // cannot simply be reused --- it has to be recomputed here. Searching the
    // unrestricted stream is what produced anchors scattered across a
    // three-hour recording and a fitted slope of -0.32: the phrases matched,
    // just in the wrong places.
    let a = anchor(truth_all, &full);
    let stream = &full[a.hyp.clone()];
    let base = a.hyp.start;

    let mut out: Vec<AnchorDrift> = anchors
        .iter()
        .map(|a| {
            let phrase = normalize(&a.following_text);
            // Try progressively shorter prefixes: transcription errors inside the
            // anchor phrase should degrade the match, not destroy it.
            //
            // The floor of three words guards against a short prefix matching
            // somewhere unrelated, but it must not exceed the phrase itself --- a
            // bare `3..=phrase.len()` is an empty range for anything shorter, so
            // such anchors would silently never match rather than matching whole.
            let floor = phrase.len().min(3).max(1);
            let media = (floor..=phrase.len()).rev().find_map(|n| {
                find_all(stream, &phrase[..n]).first().map(|i| words[base + *i].1)
            });
            AnchorDrift {
                label: a.label.clone(),
                tape: a.tape,
                tape_seconds: a.tape_seconds,
                media_seconds: media,
                residual: None,
            }
        })
        .collect();

    // Fit per tape: Tape IV restarts its clock, so pooling the two would produce
    // a meaningless line through two unrelated coordinate systems.
    let mut slope_of_longest: Option<(usize, f64)> = None;
    let tapes: Vec<u32> = {
        let mut t: Vec<u32> = out.iter().map(|d| d.tape).collect();
        t.sort_unstable();
        t.dedup();
        t
    };
    for tape in tapes {
        let pts: Vec<(f64, f64)> = out
            .iter()
            .filter(|d| d.tape == tape)
            .filter_map(|d| d.media_seconds.map(|m| (d.tape_seconds, m)))
            .collect();
        if pts.len() < 2 {
            continue;
        }
        let n = pts.len() as f64;
        let mx = pts.iter().map(|p| p.0).sum::<f64>() / n;
        let my = pts.iter().map(|p| p.1).sum::<f64>() / n;
        let num: f64 = pts.iter().map(|p| (p.0 - mx) * (p.1 - my)).sum();
        let den: f64 = pts.iter().map(|p| (p.0 - mx).powi(2)).sum();
        if den == 0.0 {
            continue;
        }
        let slope = num / den;
        let intercept = my - slope * mx;
        for d in out.iter_mut().filter(|d| d.tape == tape) {
            if let Some(m) = d.media_seconds {
                d.residual = Some(m - (slope * d.tape_seconds + intercept));
            }
        }
        if slope_of_longest.map_or(true, |(c, _)| pts.len() > c) {
            slope_of_longest = Some((pts.len(), slope));
        }
    }

    (out, slope_of_longest.map(|(_, s)| s))
}

/// Measure segment boundaries against the truth's speaker changes.
///
/// The chance baseline is the whole point. Raw "changes landing on a boundary"
/// rewards a config for nothing more than segmenting more finely --- at one word
/// per segment every change lands on a boundary. Dividing by the density-matched
/// expectation asks the question that matters: does this segmentation know
/// something about speakers, or is it just cutting often?
fn score_speaker_boundaries(
    hyp: &TranscriptionResult,
    truth_all: &[String],
    turns: &[Turn],
) -> Option<SpeakerBoundaries> {
    if turns.is_empty() || hyp.segments.is_empty() {
        return None;
    }

    // Truth token stream carrying a speaker per token. Built with the same
    // `normalize` as everything else so it lines up with `truth_all`.
    let mut speaker_of: Vec<&str> = Vec::with_capacity(truth_all.len());
    for t in turns {
        let n = normalize(&t.text).len();
        speaker_of.extend(std::iter::repeat_n(t.speaker.as_str(), n));
    }
    if speaker_of.len() != truth_all.len() {
        // The turns fixture and the clean truth have drifted; refuse to report a
        // number rather than silently aligning the wrong things.
        return None;
    }

    // Hypothesis token stream carrying its segment index.
    let mut hyp_tokens: Vec<String> = Vec::new();
    let mut seg_of: Vec<usize> = Vec::new();
    for (i, seg) in hyp.segments.iter().enumerate() {
        for t in normalize(&seg.text) {
            hyp_tokens.push(t);
            seg_of.push(i);
        }
    }
    if hyp_tokens.is_empty() {
        return None;
    }

    // Map truth positions into hypothesis positions through the edit script.
    let a = anchor(truth_all, &hyp_tokens);
    let ops = align(&truth_all[a.truth.clone()], &hyp_tokens[a.hyp.clone()]);
    let mut t2h: std::collections::HashMap<usize, usize> = Default::default();
    let (mut ti, mut hi) = (a.truth.start, a.hyp.start);
    for op in &ops {
        match op {
            Op::Match(_) => {
                t2h.insert(ti, hi);
                ti += 1;
                hi += 1;
            }
            Op::Sub { .. } => {
                ti += 1;
                hi += 1;
            }
            Op::Del(_) => ti += 1,
            Op::Ins(_) => hi += 1,
        }
    }

    let boundaries: std::collections::HashSet<usize> = (1..seg_of.len())
        .filter(|&j| seg_of[j] != seg_of[j - 1])
        .collect();

    // Only matched tokens can locate a change, so search outwards a little for
    // the nearest anchor on each side of it.
    const REACH: usize = 25;
    let (mut changes, mut covered) = (0usize, 0usize);
    for i in 1..speaker_of.len() {
        if speaker_of[i] == speaker_of[i - 1] {
            continue;
        }
        let lo = (i.saturating_sub(REACH)..i).rev().find_map(|k| t2h.get(&k).copied());
        let hi_ = (i..(i + REACH).min(speaker_of.len())).find_map(|k| t2h.get(&k).copied());
        let (Some(lo), Some(hi_)) = (lo, hi_) else { continue };
        changes += 1;
        if (lo + 1..=hi_).any(|b| boundaries.contains(&b)) {
            covered += 1;
        }
    }
    if changes == 0 {
        return None;
    }

    let segments = hyp.segments.len() as f64;
    let expected = changes as f64 * segments / hyp_tokens.len() as f64;
    Some(SpeakerBoundaries {
        changes,
        covered,
        bleed: changes - covered,
        expected_by_chance: expected,
        lift: if expected > 0.0 { covered as f64 / expected } else { 0.0 },
        mean_segment_tokens: hyp_tokens.len() as f64 / segments,
    })
}

/// True if a word's text carries a whisper.cpp control token.
///
/// whisper.cpp emits `[_BEG_]` and `[_TT_NNN]`, not the `<|...|>` form. `[_TT_]`
/// in particular carries no leading space, so it gets concatenated onto the
/// preceding word (`"you.[_TT_1499]"`) --- which is why this checks `contains`
/// rather than `starts_with`.
fn is_control_token_word(w: &str) -> bool {
    w.contains("[_")
}

fn structure_stats(hyp: &TranscriptionResult, time_slope: Option<f64>) -> StructureStats {
    let words: Vec<_> = hyp.segments.iter().flat_map(|s| s.words.iter()).collect();

    // Roughly 25 chars/sec is already far past any human speech rate; anything
    // slower than that for a single word means the timestamp is smeared.
    let implausible = words
        .iter()
        .filter(|w| {
            let d = w.end - w.start;
            d > 2.0 && d > (w.word.chars().count() as f64 / 4.0).max(2.0)
        })
        .count();

    let mut durations: Vec<f64> = hyp
        .segments
        .iter()
        .map(|s| s.end_time - s.start_time)
        .collect();
    durations.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median = durations.get(durations.len() / 2).copied().unwrap_or(0.0);

    let quantized = hyp
        .segments
        .iter()
        .filter(|s| (s.start_time % 30.0).min(30.0 - s.start_time % 30.0) < 0.1)
        .count();

    StructureStats {
        segments: hyp.segments.len(),
        words: words.len(),
        control_token_words: words.iter().filter(|w| is_control_token_word(&w.word)).count(),
        zero_duration_words: words.iter().filter(|w| w.end <= w.start).count(),
        implausible_duration_words: implausible,
        backwards_time_words: words.iter().filter(|w| w.end < w.start).count(),
        boundary_quantization: if hyp.segments.is_empty() {
            0.0
        } else {
            quantized as f64 / hyp.segments.len() as f64
        },
        median_segment_seconds: median,
        speaker: None,
        time_slope,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcription::{Segment, Word};

    fn result(segs: Vec<(&str, f64, f64, Vec<(&str, f64, f64)>)>) -> TranscriptionResult {
        TranscriptionResult {
            segments: segs
                .into_iter()
                .map(|(text, start, end, ws)| Segment {
                    speaker: None,
                    text: text.into(),
                    start_time: start,
                    end_time: end,
                    words: ws
                        .into_iter()
                        .map(|(w, s, e)| Word { word: w.into(), start: s, end: e, p: 1.0 })
                        .collect(),
                })
                .collect(),
        }
    }

    #[test]
    fn perfect_transcription_scores_zero_wer() {
        let hyp = result(vec![("act up received badly", 0.0, 2.0, vec![])]);
        let lex = Lexicon::parse("ACT UP");
        let card = score("ACT UP received badly.", &hyp, &lex, &[]);
        assert_eq!(card.wer.rate, 0.0);
        assert_eq!(card.lexicon.recall, 1.0);
    }

    #[test]
    fn lexicon_recall_reports_the_confusion() {
        // The real failure from /mnt/s3/fs1/out/002.lexical-diffs.txt.
        let hyp = result(vec![("acta up received badly", 0.0, 2.0, vec![])]);
        let lex = Lexicon::parse("ACT UP");
        let card = score("ACT UP received badly.", &hyp, &lex, &[]);
        let t = &card.lexicon.terms[0];
        assert_eq!((t.expected, t.found), (1, 0));
        assert!(t.confusions.contains(&"acta".to_string()), "{:?}", t.confusions);
        assert_eq!(card.lexicon.recall, 0.0);
    }

    #[test]
    fn lexicon_collapses_entries_that_normalise_alike() {
        let lex = Lexicon::parse("Silence = Death\nSILENCE = DEATH\nsilence equals death");
        assert_eq!(lex.terms.len(), 1, "three spellings, one term");
        assert_eq!(lex.terms[0].0, "Silence = Death", "keeps the first spelling");

        // But a genuinely different phrase stays separate.
        let lex = Lexicon::parse("Silence = Death\nSilence Death");
        assert_eq!(lex.terms.len(), 2);
    }

    #[test]
    fn terms_absent_from_the_truth_are_excluded_not_counted_as_misses() {
        let hyp = result(vec![("act up", 0.0, 1.0, vec![])]);
        let lex = Lexicon::parse("ACT UP\nWojnarowicz\nGMHC");
        let card = score("ACT UP", &hyp, &lex, &[]);
        assert_eq!(card.lexicon.terms.len(), 1);
        assert_eq!(card.lexicon.recall, 1.0);
    }

    #[test]
    fn filler_insertions_are_separated_from_content_insertions() {
        let hyp = result(vec![("well um I think obviously", 0.0, 2.0, vec![])]);
        let lex = Lexicon::parse("");
        let card = score("Well I think", &hyp, &lex, &[]);
        assert_eq!(card.wer.ins_filler, 1, "um");
        assert_eq!(card.wer.ins_content, 1, "obviously");
    }

    #[test]
    fn verbal_tics_are_classified_by_phrase_not_by_word() {
        // "you know" excised by the transcriber. Counting it as content would
        // read a tidy transcript as a hallucinating model -- but `you` and
        // `know` on their own must stay ordinary words.
        let hyp = result(vec![("it was you know difficult", 0.0, 2.0, vec![])]);
        let card = score("It was difficult", &hyp, &Lexicon::parse(""), &[]);
        assert_eq!(card.wer.ins_filler, 2, "the phrase, both tokens");
        assert_eq!(card.wer.ins_content, 0);

        // Same words, used for real: not a tic.
        let hyp = result(vec![("do you know him", 0.0, 2.0, vec![])]);
        let card = score("Do him", &hyp, &Lexicon::parse(""), &[]);
        assert_eq!(card.wer.ins_filler, 2, "still adjacent, still the tic phrase");

        let hyp = result(vec![("we know things", 0.0, 2.0, vec![])]);
        let card = score("We things", &hyp, &Lexicon::parse(""), &[]);
        assert_eq!(card.wer.ins_content, 1, "bare 'know' is an ordinary word");
    }

    #[test]
    fn stutters_count_as_filler_not_as_invented_content() {
        // "the the room" -- a false start the transcriber wrote once.
        let hyp = result(vec![("that was in the the room", 0.0, 2.0, vec![])]);
        let card = score("That was in the room", &hyp, &Lexicon::parse(""), &[]);
        assert_eq!(card.wer.ins_filler, 1);
        assert_eq!(card.wer.ins_content, 0);
    }

    #[test]
    fn detects_leaked_control_tokens() {
        let hyp = result(vec![(
            "Okay.",
            0.0,
            0.84,
            vec![("[_BEG_]", 0.0, 0.0), ("Okay.[_TT_42]", 0.0, 0.84)],
        )]);
        let card = score("Okay.", &hyp, &Lexicon::parse(""), &[]);
        assert_eq!(card.structure.control_token_words, 2);
        assert_eq!(card.structure.zero_duration_words, 1);
    }

    #[test]
    fn detects_vad_window_quantization() {
        let hyp = result(vec![
            ("a", 0.0, 0.5, vec![]),
            ("b", 30.0, 59.98, vec![]),
            ("c", 60.0, 67.5, vec![]),
        ]);
        let card = score("a b c", &hyp, &Lexicon::parse(""), &[]);
        assert_eq!(card.structure.boundary_quantization, 1.0);
    }

    #[test]
    fn drift_fit_recovers_offset_and_unit_slope() {
        // Media runs 100 s behind tape time, at correct speed.
        let words = vec![("alpha", 700.0, 700.5), ("beta", 1000.0, 1000.5)];
        let hyp = result(vec![("alpha beta", 700.0, 1000.5, words)]);
        let anchors = vec![
            AnchorSpec { tape: 3, label: "A".into(), tape_seconds: 800.0, following_text: "alpha".into() },
            AnchorSpec { tape: 3, label: "B".into(), tape_seconds: 1100.0, following_text: "beta".into() },
        ];
        let card = score("alpha beta", &hyp, &Lexicon::parse(""), &anchors);
        assert!((card.structure.time_slope.unwrap() - 1.0).abs() < 1e-9);
        assert_eq!(card.drift[0].media_seconds, Some(700.0));
        assert!(card.drift.iter().all(|d| d.residual.unwrap().abs() < 1e-9));
    }
}
