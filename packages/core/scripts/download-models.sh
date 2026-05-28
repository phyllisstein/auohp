#!/usr/bin/env bash
# Download ML models required by the auohp-core transcription pipeline.
#
# Usage:
#   ./download-models.sh                   # installs to /opt/auohp/models
#   MODELS_DIR=~/auohp/models ./download-models.sh
#   ./download-models.sh ~/auohp/models
#
# All models are public; no HuggingFace token is required.

set -euo pipefail

MODELS_DIR="${1:-${MODELS_DIR:-/opt/auohp/models}}"
mkdir -p "$MODELS_DIR"
echo "Models directory: $MODELS_DIR"

HF_BASE="https://huggingface.co"

download() {
    local url="$1"
    local dest="$2"
    if [ -f "$dest" ]; then
        echo "  exists: $(basename "$dest")"
        return
    fi
    echo "  downloading: $(basename "$dest")"
    # If HF_TOKEN is set, pass it as a Bearer token for gated model access.
    # ${VAR:+word} is a bash parameter expansion that expands to "word" only
    # when VAR is set and non-empty---so curl gets no -H flag at all when
    # there's no token, rather than an empty Authorization header.
    curl -fL --progress-bar \
        ${HF_TOKEN:+-H "Authorization: Bearer $HF_TOKEN"} \
        -o "$dest.tmp" "$url"
    mv "$dest.tmp" "$dest"
}

# ── Whisper large-v3 (GGML, ≈2.9 GB) ───────────────────────
# whisper-rs uses whisper.cpp's GGML format. large-v3 is the full 32-decoder-
# layer model (≈1.5B params)---significantly more accurate than the distilled
# turbo variant (4 layers) for proper nouns, punctuation, and disfluencies.
# Multilingual, but we force language="en" at inference time.
echo
echo "==> Whisper ggml-large-v3.bin (GGML)"
download \
    "$HF_BASE/ggerganov/whisper.cpp/resolve/main/ggml-large-v3.bin" \
    "$MODELS_DIR/ggml-large-v3.bin"

# ── silero-vad v6.2.0 (GGML, ≈2 MB) ─────────────────────────────────────────
# Used by whisper.cpp's built-in VAD to pre-segment audio before ASR.
# whisper.cpp feeds each detected speech segment to Whisper independently,
# preventing unrelated speech from merging into one segment.  Critical for
# the Q&A interview pattern (short question / very long answer).
echo
echo "==> silero-vad v6.2.0 (GGML)"
download \
    "$HF_BASE/ggml-org/whisper-vad/resolve/main/ggml-silero-v6.2.0.bin" \
    "$MODELS_DIR/ggml-silero-v6.2.0.bin"

# ── nomic-embed-text-v1.5 (ONNX, ≈275 MB) ───────────────────────────────────
# Sentence embedding model used by the search indexer.  fastembed loads it
# via UserDefinedEmbeddingModel (five flat files), so we download them here
# rather than relying on fastembed's HuggingFace Hub auto-download.
NOMIC_DIR="$MODELS_DIR/nomic-embed-text-v1.5"
mkdir -p "$NOMIC_DIR"
echo
echo "==> nomic-embed-text-v1.5 (ONNX)"
download "$HF_BASE/nomic-ai/nomic-embed-text-v1.5/resolve/main/onnx/model.onnx"              "$NOMIC_DIR/model.onnx"
download "$HF_BASE/nomic-ai/nomic-embed-text-v1.5/resolve/main/tokenizer.json"               "$NOMIC_DIR/tokenizer.json"
download "$HF_BASE/nomic-ai/nomic-embed-text-v1.5/resolve/main/tokenizer_config.json"        "$NOMIC_DIR/tokenizer_config.json"
download "$HF_BASE/nomic-ai/nomic-embed-text-v1.5/resolve/main/config.json"                  "$NOMIC_DIR/config.json"
download "$HF_BASE/nomic-ai/nomic-embed-text-v1.5/resolve/main/special_tokens_map.json"      "$NOMIC_DIR/special_tokens_map.json"

echo
echo "Done. All models in $MODELS_DIR"
