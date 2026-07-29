//! Transcription scoring harness.
//!
//! Measures a pipeline run against a human transcript. The instrument is
//! deliberately separate from the thing it measures: nothing in here may reach
//! into `crate::transcription` to change behaviour, only to read its output
//! types. An instrument that can adjust its subject can produce any reading it
//! likes.
//!
//! ## What it measures
//!
//! Scored: word error rate, and recall over a domain lexicon of proper nouns and
//! ACT UP terms of art. Reported but not optimised against: tape-anchor timing
//! drift and structural statistics.
//!
//! ## What the numbers mean
//!
//! Absolute WER is **not** a grade. The ground truth is OCR'd from a PDF and
//! editorially cleaned --- disfluencies silently removed, page furniture
//! interleaved mid-sentence, scan damage throughout --- so a flawless
//! transcription still scores poorly against it. Every figure here is a
//! *relative* signal between configs on the same fixture, and the fixtures are
//! not interchangeable with each other either.

mod align;
mod metrics;
mod normalize;

pub use align::{align, anchor, Anchoring, Op};
pub use metrics::{
    score, score_with_turns, AnchorDrift, AnchorSpec, AnchorsFile, ErrorTaxonomy, Lexicon,
    LexiconRecall, LexiconReport, Scorecard, SpeakerBoundaries, StructureStats, Turn, TurnsFile,
    WordErrorRate, HARNESS_VERSION,
};
pub use normalize::{is_filler, normalize, strip_fillers, FILLERS, FILLER_PHRASES};
