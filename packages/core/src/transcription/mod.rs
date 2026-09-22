mod audio;
mod config;
mod diarize;
mod pipeline;
mod segmentation;
mod types;
mod whisper;

// Each model's filename is owned by the module that drives that model, so the
// re-exports below disambiguate the two that are both just `MODEL_FILE` in
// their own namespace.
pub use audio::{decode_file, decode_file_with, DecodedAudio};
pub use config::{
    AudioConfig, DecodeConfig, DiarizeConfig, Interpolation, TranscribeConfig, VadConfig,
};
pub use diarize::{
    cosine_distance, diarize, dominant_speaker, extract_segment_embeddings, DiarizedSegment,
    SegmentEmbedding, EMBEDDING_MODEL_FILE,
};
pub use pipeline::{models_dir, run, run_with};
pub use segmentation::MODEL_FILE as SEGMENTATION_MODEL_FILE;
pub use types::{Segment, TranscriptionResult, Word};
pub use whisper::{MODEL_FILE as WHISPER_MODEL_FILE, VAD_MODEL_FILE};
