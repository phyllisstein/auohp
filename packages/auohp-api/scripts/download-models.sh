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
    # when VAR is set and non-empty — so curl gets no -H flag at all when
    # there's no token, rather than an empty Authorization header.
    curl -fL --progress-bar \
        ${HF_TOKEN:+-H "Authorization: Bearer $HF_TOKEN"} \
        -o "$dest.tmp" "$url"
    mv "$dest.tmp" "$dest"
}

# ── Whisper ggml-medium (GGML, ~3 GB) ─────────────────────────────────────────
# whisper-rs uses whisper.cpp's GGML format.
echo
echo "==> Whisper ggml-medium.en-q8_0.bin (GGML)"
download \
    "$HF_BASE/ggerganov/whisper.cpp/resolve/main/ggml-medium.en-q8_0.bin" \
    "$MODELS_DIR/ggml-medium.en-q8_0.bin"

# ── pyannote segmentation 3.0 (ONNX, ~17 MB) ────────────────────────────────
# Requires accepting terms at https://huggingface.co/pyannote/segmentation-3.0
# Export HF_TOKEN to authenticate.
echo
echo "==> pyannote segmentation-3.0 (ONNX)"
download \
    "$HF_BASE/onnx-community/pyannote-segmentation-3.0/resolve/main/onnx/model.onnx" \
    "$MODELS_DIR/pyannote-segmentation-3.0.onnx"

# ── wespeaker speaker embeddings (ONNX, ~26 MB) ─────────────────────────────
# VoxCeleb ResNet34-LM embeddings used by pyannote-rs for speaker clustering.
echo
echo "==> wespeaker voxceleb resnet34-LM (ONNX)"
download \
    "$HF_BASE/pyannote/wespeaker-voxceleb-resnet34-LM/resolve/main/pytorch_model.bin" \
    "$MODELS_DIR/wespeaker-voxceleb-resnet34-LM.onnx"

echo
echo "Done. All models in $MODELS_DIR"
