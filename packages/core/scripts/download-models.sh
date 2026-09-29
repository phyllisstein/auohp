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

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &>/dev/null && pwd)"
DEFAULT_MODELS_DIR="$SCRIPT_DIR/../models"

MODELS_DIR="${1:-${MODELS_DIR:-$DEFAULT_MODELS_DIR}}"
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

# ── Qwen3-Embedding-0.6B ───────────────────────────────────
# Sentence embedding model used by the search indexer.  fastembed loads it
# via UserDefinedEmbeddingModel (five flat files), so we download them here
# rather than relying on fastembed's HuggingFace Hub auto-download.

# FIXME: Export ONNX in advance to avoid runtime conversion.
# FIXME: Quantize the ONNX model to reduce memory footprint and improve inference speed.
QWEN3_DIR="$MODELS_DIR/Qwen3-Embedding-0.6B"
mkdir -p "$QWEN3_DIR"
echo
echo "==> Qwen3-Embedding-0.6B (ONNX)"

if [ ! -f "$QWEN3_DIR/config.json" ]; then
    uv tool run --from 'optimum-onnx[onnxruntime]' --with 'sentence-transformers<6' --with accelerate \
        optimum-cli export onnx \
            --model Qwen/Qwen3-Embedding-0.6B \
            --task feature-extraction \
            --library sentence_transformers \
            "$QWEN3_DIR"
fi


# ── pyannote segmentation 3.0 (ONNX, ≈6 MB) ─────────────────────────────────
# Speech/silence frame classifier used to detect speaker turn boundaries.
# This is the ONNX export from the pyannote-rs v0.1.0 release --- the
# upstream pyannote HuggingFace repo only ships a pytorch checkpoint.
# `transcription/segmentation.rs` drives it directly through `ort`.
echo
echo "==> pyannote-segmentation-3.0 (ONNX)"
download \
    "https://github.com/thewh1teagle/pyannote-rs/releases/download/v0.1.0/segmentation-3.0.onnx" \
    "$MODELS_DIR/pyannote-segmentation-3.0.onnx"

# ── wespeaker speaker embeddings (ONNX, ≈59 MB) ─────────────────────────────
# ECAPA-TDNN 1024 trained on VoxCeleb, from the official WeSpeaker
# HuggingFace org. Turns a diarized speech segment into a fixed-length
# embedding for clustering; `transcription/diarize.rs` drives it directly
# through `ort`, with `knf-rs` computing the log-mel filterbank features it
# expects.
echo
echo "==> wespeaker voxceleb ECAPA-TDNN 1024 (ONNX)"
download \
    "https://huggingface.co/Wespeaker/wespeaker-voxceleb-ecapa-tdnn1024-LM/resolve/main/voxceleb_ECAPA1024_LM.onnx" \
    "$MODELS_DIR/wespeaker_en_voxceleb_ECAPA1024.onnx"

echo
echo "Done. All models in $MODELS_DIR"
