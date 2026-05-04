#!/usr/bin/env bash
# Download ML models required by the auohp-api transcription pipeline.
#
# Usage:
#   ./download-models.sh                   # installs to /opt/auohp/models
#   MODELS_DIR=~/auohp/models ./download-models.sh
#   ./download-models.sh ~/auohp/models
#
# Gated models (pyannote segmentation) require a Hugging Face token with
# accepted terms at https://huggingface.co/pyannote/segmentation-3.0
# Export HF_TOKEN before running, or the download will be rejected.

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

# ── pyannote segmentation 3.0 (ONNX, ≈17 MB) ────────────────────────────────
# Use the model exported by pyannote-rs itself (v0.1.0 release asset).
# This export names its output tensor "output", which is what pyannote-rs 0.3.4
# hardcodes in segment.rs. The onnx-community HuggingFace export names it
# "logits" and is NOT compatible with this crate.
echo
echo "==> pyannote segmentation-3.0 (ONNX, pyannote-rs release)"
download \
    "https://github.com/thewh1teagle/pyannote-rs/releases/download/v0.1.0/segmentation-3.0.onnx" \
    "$MODELS_DIR/pyannote-segmentation-3.0.onnx"

# ── wespeaker speaker embeddings (ONNX, ≈59 MB) ─────────────────────────────
# ECAPA-TDNN 1024 trained on VoxCeleb, from the official WeSpeaker HuggingFace
# org. All WeSpeaker ONNX exports share the same tensor interface ("feats" →
# "embs"), so this is a drop-in replacement for the CAM++ model that pyannote-rs
# originally shipped. ECAPA-TDNN 1024 scores meaningfully lower EER on
# VoxCeleb1 (~3.8%) than CAM++ (~4.5%) or ECAPA-TDNN 512 (~4.3%).
echo
echo "==> wespeaker voxceleb ECAPA-TDNN 1024 (ONNX, WeSpeaker HuggingFace)"
download \
    "https://huggingface.co/Wespeaker/wespeaker-voxceleb-ecapa-tdnn1024-LM/resolve/main/voxceleb_ECAPA1024_LM.onnx" \
    "$MODELS_DIR/wespeaker_en_voxceleb_ECAPA1024.onnx"

echo
echo "Done. All models in $MODELS_DIR"
