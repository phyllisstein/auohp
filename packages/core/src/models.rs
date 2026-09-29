use std::path::PathBuf;

/// Where `scripts/download-models.sh` installs models when `$MODELS_DIR` is
/// unset. Each module owns the *filename* of the model it drives
/// ([`whisper::MODEL_FILE`], [`segmentation::MODEL_FILE`], and so on); this
/// only resolves the directory they all sit in.
fn default_models_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../models")
}

/// Resolve the models directory from `$MODELS_DIR`, falling back to the
/// default models directory computed at runtime (not baked in at build
/// time, since `CARGO_MANIFEST_DIR` may not reflect where the binary is
/// actually deployed).
///
/// Public because the crate's validation examples load the same models from
/// the same place; duplicating the env-var lookup there is how a harness ends
/// up silently scoring a different model than the pipeline runs.
pub fn models_dir() -> PathBuf {
    std::env::var("MODELS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| default_models_dir())
}
