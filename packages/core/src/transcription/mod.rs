mod audio;
mod config;
mod diarize;
mod pipeline;
mod types;
mod whisper;

// Each model's filename is owned by the module that drives that model, so the
// re-exports below disambiguate the two that are both just `MODEL_FILE` in
// their own namespace.
pub use audio::{DecodedAudio, decode_file, decode_file_with};
pub use config::{
    AudioConfig, DecodeConfig, DiarizeConfig, Interpolation, TranscribeConfig, TranscriptionConfig,
    VadConfig,
};
pub use diarize::segmentation::MODEL_FILE as SEGMENTATION_MODEL_FILE;
pub use diarize::{
    DiarizedSegment, EMBEDDING_MODEL_FILE, SegmentEmbedding, cosine_distance, diarize,
    dominant_speaker, extract_segment_embeddings,
};
pub use pipeline::{run, run_with};
pub use types::{ModelConfig, Segment, TranscriptionResult, Word};
pub use whisper::{MODEL_FILE as WHISPER_MODEL_FILE, VAD_MODEL_FILE};
