//! Anchoring and alignment of two token streams.
//!
//! One Levenshtein pass yields every scored metric: edit distance is WER, `Sub`
//! ops are the error taxonomy, and the aligned pairs give lexicon recall, so the
//! three cannot disagree.

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
/// Both sides are clamped: either stream may extend past the other, and
/// clamping one only turns the other's surplus into phantom insertions and a
/// WER above 1.0.
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
/// The truth excerpt starts mid-tape, so aligning from index 0 gives a garbage
/// edit script.
///
/// Diagonal voting: if `hyp[i]` and `truth[p]` are the same rare word, that is
/// weak evidence for offset `p - i`, and the true alignment shows up as a spike.
/// Common tokens vote for every offset equally, so they are excluded.
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

    // Tokens more frequent than this carry no positional information; down-weighting
    // instead of excluding leaves a noise floor that buries the spike.
    const RARE: usize = 4;

    // truth_index = hyp_index + offset; negative when the hypothesis starts
    // before the truth excerpt.
    //
    // Insertions and deletions shift the offset along the stream, so each
    // candidate is pooled with its neighbouring diagonals.
    //
    // Each side can be windowed independently: a truth tail with no hypothesis
    // counterpart votes noise, so the end may instead be estimated from the
    // hypothesis tail.
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

    // Estimate each end from both streams and keep the stronger vote.
    let pick = |a: (isize, usize), b: (isize, usize)| if a.1 >= b.1 { a } else { b };
    let (off_start, c_start) = pick(vote(0..PROBE.min(n), 0..m), vote(0..n, 0..PROBE.min(m)));
    let (off_end, c_end) = pick(
        vote(n.saturating_sub(PROBE)..n, 0..m),
        vote(0..n, m.saturating_sub(PROBE)..m),
    );

    // Only large non-overlapping prefixes/suffixes are stripped; the slack keeps
    // the clamp off shared material when the offset is slightly wrong.
    const SLACK: usize = 32;
    const FLOOR: usize = 6;

    if c_start < FLOOR && c_end < FLOOR {
        // No evidence of a non-overlapping region: return both streams whole.
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
    let truth_end = ((m as isize + off_end).max(0) as usize + SLACK).min(n).max(truth_start);
    let hyp_end = ((n as isize - off_end).max(0) as usize + SLACK).min(m).max(hyp_start);

    // Partial: more than 10% of either side was excluded.
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
/// Backpointers cover the full `truth.len() * hyp.len()` matrix (tens of MB at a
/// few thousand tokens a side); cost rows are two rolling vectors.
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

            // Ties go to Diag: a substitution says what the model heard instead.
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
        let ops = align(&toks("x"), &toks("y"));
        assert_eq!(ops, vec![Op::Sub { truth: "x".into(), hyp: "y".into() }]);
    }

    #[test]
    fn anchor_finds_a_hypothesis_that_starts_late_in_the_truth() {
        let truth = toks(
            "wojnarowicz maggenti episalla letraset signorile nesline staley \
             the quick brown fox jumped over the lazy dog near astor place",
        );
        let hyp = toks("the quick brown fox jumped over the lazy dog near astor place");
        let a = anchor(&truth, &hyp);
        // Streams this short sit inside the slack margin, so nothing is cut.
        assert!(a.truth.contains(&7), "the shared span must survive the clamp");
        assert_eq!(a.hyp, 0..hyp.len());
        assert!(a.confidence > 0);
    }

    #[test]
    fn anchor_clamps_the_hypothesis_when_it_is_the_longer_stream() {
        // Padding is well past the slack margin so the clamp has work to do.
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
