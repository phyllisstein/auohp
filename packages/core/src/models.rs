use std::path::PathBuf;

/// Where `scripts/download-models.py` installs models when `$MODELS_DIR` is
/// unset. Each module owns the filename of its model; this resolves the
/// directory.
// FIXME: `env!` expands at compile time, so a relocated binary looks for
// models in the build machine's source tree. Settle on a real default and
// change `scripts/download-models.py` to match.
fn default_models_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("models")
}

/// Resolve the models directory from `$MODELS_DIR`, falling back to
/// `<crate>/models` fixed at build time.
pub fn models_dir() -> PathBuf {
    std::env::var("MODELS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| default_models_dir())
}
