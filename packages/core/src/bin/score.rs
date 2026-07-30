//! Score a transcription run against the human transcript.
//!
//! CPU-only and cheap --- it never loads a model or touches the GPU, which is
//! what makes re-scoring every archived run after a metric change practical.
//!
//! Usage:
//!   cargo run --release --bin score -- \
//!     --truth   packages/core/tests/fixtures/108_truth.clean.txt \
//!     --hyp     /mnt/s3/fs1/runs/000-original/result.json \
//!     --lexicon packages/core/tests/fixtures/actup_lexicon.txt \
//!     --anchors packages/core/tests/fixtures/108_truth.anchors.json

use anyhow::{Context, Result};
use clap::Parser;
use std::path::PathBuf;

use auohp_core::eval::{score_with_turns, AnchorsFile, Lexicon, Scorecard, TurnsFile};
use auohp_core::transcription::TranscriptionResult;

#[derive(Parser, Debug)]
#[command(about = "Score a transcription against a human transcript")]
struct Cli {
    /// Cleaned human transcript.
    #[arg(long)]
    truth: PathBuf,

    /// Pipeline output (`result.json`).
    #[arg(long)]
    hyp: PathBuf,

    /// Domain lexicon; terms absent from the truth are skipped.
    #[arg(long)]
    lexicon: Option<PathBuf>,

    /// Tape markers for timing drift.
    #[arg(long)]
    anchors: Option<PathBuf>,

    /// Speaker turns, for measuring segment boundaries against speaker changes.
    #[arg(long)]
    turns: Option<PathBuf>,

    /// Emit JSON instead of the human summary.
    #[arg(long)]
    json: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let truth = std::fs::read_to_string(&cli.truth)
        .with_context(|| format!("failed to read truth {}", cli.truth.display()))?;
    let hyp_raw = std::fs::read_to_string(&cli.hyp)
        .with_context(|| format!("failed to read hypothesis {}", cli.hyp.display()))?;
    let hyp: TranscriptionResult =
        serde_json::from_str(&hyp_raw).context("hypothesis is not a TranscriptionResult")?;

    let lexicon = match &cli.lexicon {
        Some(p) => Lexicon::parse(
            &std::fs::read_to_string(p)
                .with_context(|| format!("failed to read lexicon {}", p.display()))?,
        ),
        None => Lexicon::parse(""),
    };

    let anchors = match &cli.anchors {
        Some(p) => {
            let raw = std::fs::read_to_string(p)
                .with_context(|| format!("failed to read anchors {}", p.display()))?;
            serde_json::from_str::<AnchorsFile>(&raw)
                .context("anchors file is malformed")?
                .anchors
        }
        None => vec![],
    };

    let turns = match &cli.turns {
        Some(p) => {
            let raw = std::fs::read_to_string(p)
                .with_context(|| format!("failed to read turns {}", p.display()))?;
            serde_json::from_str::<TurnsFile>(&raw)
                .context("turns file is malformed")?
                .turns
        }
        None => vec![],
    };

    let card = score_with_turns(&truth, &hyp, &lexicon, &anchors, &turns);

    if cli.json {
        println!("{}", serde_json::to_string_pretty(&card)?);
    } else {
        print_summary(&card);
    }
    Ok(())
}

fn print_summary(c: &Scorecard) {
    println!("harness v{}", c.harness_version);
    println!();

    if c.partial_coverage {
        println!(
            "  ** partial coverage: the transcript and the clip cover different extents."
        );
        println!(
            "     Only their overlap was scored -- truth[{}..{}] against hyp[{}..{}].",
            c.anchored_truth_range.0, c.anchored_truth_range.1,
            c.anchored_hyp_range.0, c.anchored_hyp_range.1
        );
        println!("     Numbers below describe that overlap, not the whole file.");
        println!();
    }

    if c.anchor_confidence < 10 {
        println!(
            "  !! anchor confidence {} --- truth and hypothesis may not be the same recording.",
            c.anchor_confidence
        );
        println!("     Treat every number below as unreliable until this is resolved.");
        println!();
    }

    let w = &c.wer;
    println!("WORD ERROR RATE  {:.4}   (relative signal only --- not an accuracy grade)", w.rate);
    println!(
        "  {} sub   {} del   {} ins ({} filler, {} content)",
        w.subs, w.dels, w.ins_filler + w.ins_content, w.ins_filler, w.ins_content
    );
    println!(
        "  compared truth[{}..{}] against hyp[{}..{}]   ({} vs {} tokens)",
        c.anchored_truth_range.0,
        c.anchored_truth_range.1,
        c.anchored_hyp_range.0,
        c.anchored_hyp_range.1,
        w.truth_tokens,
        w.hyp_tokens
    );
    println!();

    println!("LEXICON RECALL   {:.4}   over {} terms present in the truth", c.lexicon.recall, c.lexicon.terms.len());
    for t in c.lexicon.terms.iter().take(12) {
        if t.found < t.expected {
            let conf = if t.confusions.is_empty() {
                String::new()
            } else {
                format!("   heard as: {}", t.confusions.join(", "))
            };
            println!("  {:>2}/{:<2}  {}{}", t.found, t.expected, t.term, conf);
        }
    }
    println!();

    let s = &c.structure;
    println!("STRUCTURE");
    println!("  {} segments, {} words, median segment {:.1}s", s.segments, s.words, s.median_segment_seconds);
    let flag = |n: usize| if n > 0 { " <-- should be 0" } else { "" };
    println!("  control-token words      {}{}", s.control_token_words, flag(s.control_token_words));
    println!("  zero-duration words      {}", s.zero_duration_words);
    println!("  backwards-time words     {}{}", s.backwards_time_words, flag(s.backwards_time_words));
    println!("  implausible durations    {}", s.implausible_duration_words);
    println!("  boundary quantization    {:.3}   (high = following VAD windows, not speech)", s.boundary_quantization);
    if let Some(sp) = &s.speaker {
        println!(
            "  speaker-change boundaries {}/{} covered ({} bleed), {:.1} expected by chance",
            sp.covered, sp.changes, sp.bleed, sp.expected_by_chance
        );
        println!(
            "  speaker-boundary lift     {:.2}x  (1.0 = segmentation knows nothing about speakers;\n                             read this, not the raw count -- finer segments inflate it)",
            sp.lift
        );
        println!("  mean segment              {:.1} tokens", sp.mean_segment_tokens);
    }

    match s.time_slope {
        Some(sl) => {
            // The anchors come from timecodes printed in a PDF margin beside a
            // line of text, so locating one is only accurate to the nearest line
            // -- tens of seconds. Over a 25-minute span that is several percent
            // of slope on its own, which is why the tolerance is this wide.
            // Below it, the slope says nothing; above it, look at the residuals
            // for a monotonic trend before concluding anything, since scattered
            // residuals are placement noise and only a trend is real drift.
            let note = if (sl - 1.0).abs() > 0.10 {
                "  <-- well off 1.0; check residuals below for a monotonic trend"
            } else {
                "  (within anchor-placement noise)"
            };
            println!("  tape-time slope          {:.4}{}", sl, note);
        }
        None => println!("  tape-time slope          n/a (too few located anchors)"),
    }

    let t = &c.taxonomy;
    if !t.substitutions.is_empty() || !t.insertions.is_empty() {
        println!();
        println!("TOP ERRORS");
        let show = |label: &str, rows: Vec<String>| {
            if !rows.is_empty() {
                println!("  {label}");
                for r in rows.iter().take(10) {
                    println!("    {r}");
                }
            }
        };
        show(
            "substitutions (truth -> heard)",
            t.substitutions.iter().map(|(a, b, n)| format!("{n:>4}  {a} -> {b}")).collect(),
        );
        show(
            "insertions (added by the model)",
            t.insertions.iter().map(|(a, n)| format!("{n:>4}  {a}")).collect(),
        );
        show(
            "deletions (dropped by the model)",
            t.deletions.iter().map(|(a, n)| format!("{n:>4}  {a}")).collect(),
        );
    }

    if !c.drift.is_empty() {
        println!();
        println!("TIMING ANCHORS");
        for d in &c.drift {
            match (d.media_seconds, d.residual) {
                (Some(m), Some(r)) => println!("  {:<20} tape {:>6.0}s -> media {:>7.1}s   residual {:+.1}s", d.label, d.tape_seconds, m, r),
                (Some(m), None) => println!("  {:<20} tape {:>6.0}s -> media {:>7.1}s", d.label, d.tape_seconds, m),
                _ => println!("  {:<20} tape {:>6.0}s -> NOT FOUND in hypothesis", d.label, d.tape_seconds),
            }
        }
    }
}
