//! Transcription scoring harness.
//!
//! Measures a pipeline run against a human transcript. Nothing here may change
//! `crate::transcription` behaviour; it only reads its output types.
//!
//! Scored: word error rate, and recall over a domain lexicon of proper nouns and
//! ACT UP terms of art. Reported only: tape-anchor timing drift and structural
//! statistics.
//!
//! Absolute WER is not a grade. The ground truth is OCR'd from a PDF and
//! editorially cleaned (disfluencies removed, page furniture interleaved,
//! scan damage), so a flawless transcription still scores poorly. Figures are
//! relative signals between configs on the same fixture.

mod align;
pub mod diarization;
mod metrics;
mod normalize;

pub use align::{align, anchor, Anchoring, Op};
pub use metrics::{
    score, score_with_turns, AnchorDrift, AnchorSpec, AnchorsFile, ErrorTaxonomy, Lexicon,
    LexiconRecall, LexiconReport, Scorecard, SpeakerBoundaries, StructureStats, Turn, TurnsFile,
    WordErrorRate, HARNESS_VERSION,
};
pub use normalize::{is_filler, normalize, strip_fillers, FILLERS, FILLER_PHRASES};
