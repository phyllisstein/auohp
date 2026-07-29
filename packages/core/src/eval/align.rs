//! Anchoring and alignment of two token streams.
//!
//! This module is the whole instrument. One Levenshtein pass produces every
//! scored metric at once: the edit distance is WER, the `Sub` ops are the error
//! taxonomy, and scanning the aligned pairs gives per-term lexicon recall.
//! Computing those separately over the raw text would let them disagree at the
//! margins --- and the disagreement would be invisible, because each number
//! would look plausible on its own.

use std::collections::HashMap;
use std::ops::Range;

/// One edit in the script that turns the truth stream into the hypothesis stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Op {
    /// Both sides agree. Carries the token so callers can scan matches for
    /// lexicon terms without re-indexing the streams.
    Match(String),
    /// Both sides committed to a word and disagreed. The most trustworthy
    /// evidence of a real model error.
    Sub { truth: String, hyp: String },
    /// Present in the hypothesis, absent from the truth. Frequently a disfluency
    /// the human transcriber silently dropped rather than a model hallucination
    /// --- the scorer separates those two before drawing conclusions.
    Ins(String),
    /// Present in the truth, absent from the hypothesis.
    Del(String),
}

/// The overlapping windows of the two streams.
///
/// Both sides are clamped, not just the truth. Either stream may extend past the
/// other in either direction: the ground truth is an excerpt that starts
/// mid-tape, and a hypothesis may cover a whole three-hour interview of which the
/// excerpt is a middle slice. Clamping one side only turns the other side's
/// surplus into tens of thousands of phantom insertions and a WER above 1.0 that
/// looks like total model failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Anchoring {
    pub truth: Range<usize>,
    pub hyp: Range<usize>,
    /// Votes backing the winning diagonal. Low confidence means the streams may
    /// not be the same recording at all, and the caller should say so rather
    /// than reporting a confident WER.
    pub confidence: usize,
    /// One stream extends well past the other, so only the overlap was aligned.
    /// Normal for this corpus: a transcript often covers more tape than the clip
    /// drawn from it. Scores still describe the overlap honestly, but they
    /// describe *less material* than the fixture names suggest.
    pub partial_coverage: bool,
}

/// Find the slice of `truth` that `hyp` actually covers.
///
/// The ground truth for this corpus does not begin where the audio begins: the
/// transcript excerpt starts mid-tape. Aligning from index 0 yields a garbage
/// edit script and a WER near 1.0 that reads as catastrophic model failure when
/// it is really a bookkeeping error.
///
/// The method is diagonal voting. If `hyp[i]` and `truth[p]` are the same *rare*
/// word, that is weak evidence for the offset `p - i`; accumulate the votes and
/// the true alignment shows up as a spike. Rarity is what makes this work ---
/// "the" appears everywhere and votes for every offset equally, so common tokens
/// are excluded rather than merely down-weighted.
pub fn anchor(truth: &[String], hyp: &[String]) -> Anchoring {
    if truth.is_empty() || hyp.is_empty() {
        return Anchoring {
            truth: 0..truth.len(),
            hyp: 0..hyp.len(),
            confidence: 0,
            partial_coverage: false,
        };
    }

    let mut positions: HashMap<&str, Vec<usize>> = HashMap::new();
    for (i, t) in truth.iter().enumerate() {
        positions.entry(t.as_str()).or_default().push(i);
    }

    // Tokens appearing more than this often in the truth carry no positional
    // information: "the" sits on every diagonal and votes for all of them
    // equally. Excluding them outright is what makes the spike visible --- mere
    // down-weighting leaves a noise floor that buries it on long streams.
    const RARE: usize = 4;

    // truth_index = hyp_index + offset. Negative when the hypothesis starts
    // before the truth excerpt does, which is the common case here.
    //
    // Votes land on adjacent diagonals rather than exactly one, because real
    // streams differ by insertions and deletions that shift the offset as you go.
    // Pooling each candidate with its immediate neighbours recovers the true peak
    // instead of picking whichever single diagonal happened to win a split vote.
    // Votes restricted to a window on *each* side. Windowing only the truth was
    // the flaw: when a transcript covers more tape than the clip drawn from it,
    // the truth's tail has no counterpart, its vote is noise, and there is no way
    // to tell that from a low count alone. Being able to vote from the
    // hypothesis's tail instead means the end is always estimated from material
    // that is actually present in both streams.
    let vote = |tw: std::ops::Range<usize>, hw: std::ops::Range<usize>| -> (isize, usize) {
        let mut votes: HashMap<isize, usize> = HashMap::new();
        for j in hw.clone() {
            let Some(tok) = hyp.get(j) else { continue };
            if let Some(ps) = positions.get(tok.as_str()) {
                if ps.len() <= RARE {
                    for p in ps.iter().filter(|p| tw.contains(p)) {
                        *votes.entry(*p as isize - j as isize).or_default() += 1;
                    }
                }
            }
        }
        votes
            .keys()
            .map(|&off| {
                let pooled: usize = (-2..=2).filter_map(|d| votes.get(&(off + d))).sum();
                (off, pooled)
            })
            .max_by_key(|(_, n)| *n)
            .unwrap_or((0, 0))
    };

    const PROBE: usize = 400;
    let (n, m) = (truth.len(), hyp.len());

    // Estimate each end twice --- once anchored on the truth, once on the
    // hypothesis --- and keep whichever voted more strongly. Whichever stream is
    // the clipped one, the other still has covered material at that end.
    let pick = |a: (isize, usize), b: (isize, usize)| if a.1 >= b.1 { a } else { b };
    let (off_start, c_start) = pick(vote(0..PROBE.min(n), 0..m), vote(0..n, 0..PROBE.min(m)));
    let (off_end, c_end) = pick(
        vote(n.saturating_sub(PROBE)..n, 0..m),
        vote(0..n, m.saturating_sub(PROBE)..m),
    );

    // Anchoring exists to strip a large non-overlapping prefix or suffix, not to
    // trim a few tokens --- the DP handles small differences correctly on its own.
    // A slack margin keeps the clamp from cutting into genuinely shared material
    // when the offset is off by a little.
    const SLACK: usize = 32;
    const FLOOR: usize = 6;

    if c_start < FLOOR && c_end < FLOOR {
        // No usable evidence of a non-overlapping region. Inventing a clamp here
        // would discard real material, so hand both streams back whole and let
        // the reported confidence say the alignment is untrustworthy.
        return Anchoring {
            truth: 0..n,
            hyp: 0..m,
            confidence: c_start.max(c_end),
            partial_coverage: false,
        };
    }

    // Fall back to the better-supported end if one of them voted weakly.
    let off_start = if c_start >= FLOOR { off_start } else { off_end };
    let off_end = if c_end >= FLOOR { off_end } else { off_start };

    let truth_start = (off_start.max(0) as usize).saturating_sub(SLACK);
    let hyp_start = ((-off_start).max(0) as usize).saturating_sub(SLACK);
    // Under `truth_index = hyp_index + off_end`, the overlap ends where whichever
    // stream runs out first does.
    let truth_end = ((m as isize + off_end).max(0) as usize + SLACK).min(n).max(truth_start);
    let hyp_end = ((n as isize - off_end).max(0) as usize + SLACK).min(m).max(hyp_start);

    // "Partial" means a real chunk of one side was excluded, not that the edges
    // were tidied. Reported so a scorecard cannot silently describe half a file.
    let partial = (truth_end - truth_start) * 10 < n * 9 || (hyp_end - hyp_start) * 10 < m * 9;

    Anchoring {
        truth: truth_start..truth_end.max(truth_start),
        hyp: hyp_start..hyp_end.max(hyp_start),
        // Report the end we actually relied on, not the unusable one.
        confidence: c_start.min(c_end),
        partial_coverage: partial,
    }
}

/// Levenshtein edit script over token streams, with equal cost for all three
/// edit kinds --- the standard WER convention.
///
/// Backpointers are stored for the full matrix so the script can be recovered.
/// That is `truth.len() * hyp.len()` bytes; at oral-history scale (a few
/// thousand tokens a side) it is tens of megabytes, which is worth spending to
/// keep the traceback exact. The cost rows are kept as two rolling vectors
/// because only the previous row is ever needed.
pub fn align(truth: &[String], hyp: &[String]) -> Vec<Op> {
    let (n, m) = (truth.len(), hyp.len());
    if n == 0 {
        return hyp.iter().map(|h| Op::Ins(h.clone())).collect();
    }
    if m == 0 {
        return truth.iter().map(|t| Op::Del(t.clone())).collect();
    }

    #[derive(Clone, Copy, PartialEq)]
    enum Bp {
        Diag,
        Up,
        Left,
    }

    let mut bp = vec![Bp::Diag; (n + 1) * (m + 1)];
    let idx = |i: usize, j: usize| i * (m + 1) + j;

    let mut prev: Vec<u32> = (0..=m as u32).collect();
    let mut cur: Vec<u32> = vec![0; m + 1];

    for j in 1..=m {
        bp[idx(0, j)] = Bp::Left;
    }
    for i in 1..=n {
        bp[idx(i, 0)] = Bp::Up;
    }

    for i in 1..=n {
        cur[0] = i as u32;
        for j in 1..=m {
            let sub_cost = prev[j - 1] + u32::from(truth[i - 1] != hyp[j - 1]);
            let del_cost = prev[j] + 1; // consume truth only
            let ins_cost = cur[j - 1] + 1; // consume hyp only

            // Tie-break toward Diag so equal-cost paths prefer substitutions over
            // an insert/delete pair. Both are distance 1, but a substitution says
            // "the model heard this word as that word" --- which is a usable
            // finding --- while ins+del says only that something changed.
            let (best, b) = if sub_cost <= del_cost && sub_cost <= ins_cost {
                (sub_cost, Bp::Diag)
            } else if del_cost <= ins_cost {
                (del_cost, Bp::Up)
            } else {
                (ins_cost, Bp::Left)
            };
            cur[j] = best;
            bp[idx(i, j)] = b;
        }
        std::mem::swap(&mut prev, &mut cur);
    }

    let (mut i, mut j) = (n, m);
    let mut ops = Vec::new();
    while i > 0 || j > 0 {
        match (i, j) {
            (0, _) => {
                ops.push(Op::Ins(hyp[j - 1].clone()));
                j -= 1;
            }
            (_, 0) => {
                ops.push(Op::Del(truth[i - 1].clone()));
                i -= 1;
            }
            _ => match bp[idx(i, j)] {
                Bp::Diag => {
                    ops.push(if truth[i - 1] == hyp[j - 1] {
                        Op::Match(truth[i - 1].clone())
                    } else {
                        Op::Sub { truth: truth[i - 1].clone(), hyp: hyp[j - 1].clone() }
                    });
                    i -= 1;
                    j -= 1;
                }
                Bp::Up => {
                    ops.push(Op::Del(truth[i - 1].clone()));
                    i -= 1;
                }
                Bp::Left => {
                    ops.push(Op::Ins(hyp[j - 1].clone()));
                    j -= 1;
                }
            },
        }
    }
    ops.reverse();
    ops
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toks(s: &str) -> Vec<String> {
        s.split_whitespace().map(String::from).collect()
    }

    #[test]
    fn identical_streams_are_all_matches() {
        let t = toks("the quick brown fox");
        let ops = align(&t, &t);
        assert_eq!(ops.len(), 4);
        assert!(ops.iter().all(|o| matches!(o, Op::Match(_))));
    }

    #[test]
    fn one_substitution() {
        let ops = align(&toks("act up received badly"), &toks("acta up received badly"));
        assert_eq!(
            ops[0],
            Op::Sub { truth: "act".into(), hyp: "acta".into() }
        );
        assert_eq!(ops[1..].iter().filter(|o| matches!(o, Op::Match(_))).count(), 3);
    }

    #[test]
    fn insertion_and_deletion_are_directional() {
        // hyp has an extra token -> insertion.
        let ops = align(&toks("a b"), &toks("a um b"));
        assert_eq!(ops, vec![Op::Match("a".into()), Op::Ins("um".into()), Op::Match("b".into())]);

        // truth has a token the hyp lacks -> deletion.
        let ops = align(&toks("a um b"), &toks("a b"));
        assert_eq!(ops, vec![Op::Match("a".into()), Op::Del("um".into()), Op::Match("b".into())]);
    }

    #[test]
    fn empty_sides() {
        assert_eq!(align(&[], &toks("a b")), vec![Op::Ins("a".into()), Op::Ins("b".into())]);
        assert_eq!(align(&toks("a b"), &[]), vec![Op::Del("a".into()), Op::Del("b".into())]);
    }

    #[test]
    fn hand_worked_edit_script() {
        // truth: the cat sat on the mat
        // hyp:   the cat sat on a mat
        // Exactly one substitution (the -> a); everything else matches.
        let ops = align(&toks("the cat sat on the mat"), &toks("the cat sat on a mat"));
        let subs: Vec<_> = ops
            .iter()
            .filter_map(|o| match o {
                Op::Sub { truth, hyp } => Some((truth.as_str(), hyp.as_str())),
                _ => None,
            })
            .collect();
        assert_eq!(subs, vec![("the", "a")]);
        assert_eq!(ops.iter().filter(|o| matches!(o, Op::Match(_))).count(), 5);
    }

    #[test]
    fn equal_cost_paths_prefer_substitution_over_ins_plus_del() {
        // "x" -> "y" is distance 1 either way; we want it reported as a Sub,
        // because "heard x as y" is actionable and "something changed" is not.
        let ops = align(&toks("x"), &toks("y"));
        assert_eq!(ops, vec![Op::Sub { truth: "x".into(), hyp: "y".into() }]);
    }

    #[test]
    fn anchor_finds_a_hypothesis_that_starts_late_in_the_truth() {
        // The truth transcript begins before the audio does.
        let truth = toks(
            "wojnarowicz maggenti episalla letraset signorile nesline staley \
             the quick brown fox jumped over the lazy dog near astor place",
        );
        let hyp = toks("the quick brown fox jumped over the lazy dog near astor place");
        let a = anchor(&truth, &hyp);
        // Streams this short sit entirely inside the slack margin, so nothing is
        // cut -- which is correct: there is no large prefix here worth stripping.
        assert!(a.truth.contains(&7), "the shared span must survive the clamp");
        assert_eq!(a.hyp, 0..hyp.len());
        assert!(a.confidence > 0);
    }

    #[test]
    fn anchor_clamps_the_hypothesis_when_it_is_the_longer_stream() {
        // The real case that a truth-only clamp got wrong: a short excerpt sitting
        // in the middle of a much longer transcription. Both sides must be cut.
        // Padding well past the slack margin, so the clamp has real work to do.
        let excerpt = "wojnarowicz maggenti episalla letraset signorile nesline staley";
        let pad = |w: &str| (0..300).map(|i| format!("{w}{i}")).collect::<Vec<_>>().join(" ");
        let truth = toks(excerpt);
        let hyp = toks(&format!("{} {excerpt} {}", pad("before"), pad("after")));

        let a = anchor(&truth, &hyp);
        assert_eq!(a.truth, 0..7, "the whole excerpt is covered");
        assert!(
            a.hyp.start > 200 && a.hyp.start <= 300,
            "hypothesis prefix must be stripped, got {:?}",
            a.hyp
        );
        assert!(
            a.hyp.end < 400,
            "hypothesis suffix must be stripped too, got {:?}",
            a.hyp
        );
        assert!(a.confidence > 0);
    }

    #[test]
    fn anchor_handles_a_truth_that_extends_past_the_hypothesis() {
        // The real 026 case: the transcript covers about twice the tape the clip
        // does, so the truth's tail has no counterpart and its vote is noise.
        // Believing that vote collapsed the window to a sixth of the true overlap.
        let shared = (0..400).map(|i| format!("shared{i}")).collect::<Vec<_>>().join(" ");
        let beyond = (0..400).map(|i| format!("beyond{i}")).collect::<Vec<_>>().join(" ");
        let truth = toks(&format!("{shared} {beyond}"));
        let hyp = toks(&shared);

        let a = anchor(&truth, &hyp);
        assert!(a.partial_coverage, "must report that coverage is partial");
        assert_eq!(a.hyp, 0..hyp.len(), "the whole hypothesis is covered");
        assert!(
            a.truth.end <= 480,
            "truth must be clamped near the overlap, not run into the tail: {:?}",
            a.truth
        );
        assert!(a.truth.len() >= 380, "and must not be over-clamped: {:?}", a.truth);
    }

    #[test]
    fn anchor_on_aligned_streams_is_the_identity_range() {
        let t = toks("wojnarowicz maggenti episalla letraset signorile nesline staley");
        let a = anchor(&t, &t);
        assert_eq!(a.truth, 0..t.len());
        assert_eq!(a.hyp, 0..t.len());
        assert!(!a.partial_coverage);
    }

    #[test]
    fn anchor_reports_no_confidence_on_unrelated_streams() {
        let t = toks("wojnarowicz maggenti episalla letraset");
        let h = toks("entirely different words appear here");
        assert_eq!(anchor(&t, &h).confidence, 0);
    }
}
