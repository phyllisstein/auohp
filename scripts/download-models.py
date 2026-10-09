#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12,<3.14"
# dependencies = [
#     "accelerate",
#     "httpx>=0.28",
#     "onnx-ir",
#     "onnxruntime>=1.22,<1.29",
#     "optimum-onnx[onnxruntime]>=0.1",
#     "sentence-transformers>=5,<6",
#     "torch>=2.6",
#     "tqdm>=4.66",
# ]
#
# [tool.uv]
# exclude-newer = "2026-10-01T00:00:00Z"
#
# [tool.uv.sources]
# torch = [{ index = "pytorch-cpu", marker = "sys_platform == 'linux'" }]
#
# [[tool.uv.index]]
# name = "pytorch-cpu"
# url = "https://download.pytorch.org/whl/cpu"
# explicit = true
# ///
"""Download and prepare the ML models required by the auohp-core pipeline.

Usage:
    ./download-models.py                    # installs to packages/core/models
    MODELS_DIR=~/auohp/models ./download-models.py
    ./download-models.py ~/auohp/models

All models are public; no HuggingFace token is required. If `HF_TOKEN` is
set, it is sent to huggingface.co only.

Dependency notes:
- `onnxruntime` is capped at the version `ort` 2.0.0-rc.13 bundles (1.28), so
  the quantizer never emits an op the Rust runtime cannot load.
- `onnx-ir` is an undeclared import of onnxruntime 1.28's
  `matmul_nbits_quantizer`.
- On Linux, torch resolves from the CPU-only index; the export runs on CPU and
  the default wheel drags in several GB of CUDA libraries.
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import sys
from pathlib import Path
from urllib.parse import urlparse

import httpx
from tqdm import tqdm

REPO_ROOT = Path(__file__).resolve().parent.parent
# FIXME: Mirrors `default_models_dir()` in `packages/core/src/models.rs`,
# which resolves to the build machine's source tree. Settle on a real default
# there and change this to match.
DEFAULT_MODELS_DIR = REPO_ROOT / "packages" / "core" / "models"

HF_BASE = "https://huggingface.co"


def resolve_models_dir(argv_dir: str | None) -> Path:
    raw = argv_dir or os.environ.get("MODELS_DIR") or DEFAULT_MODELS_DIR
    return Path(raw).expanduser().resolve()


def download(url: str, dest: Path) -> None:
    """Stream `url` to `dest`, atomically, skipping if `dest` already exists."""
    if dest.exists():
        print(f"  exists: {dest.name}")
        return

    # The token is scoped to huggingface.co; GitHub release assets (and
    # whatever CDN a redirect lands on) never see it.
    headers = {}
    token = os.environ.get("HF_TOKEN")
    if token and urlparse(url).hostname == "huggingface.co":
        headers["Authorization"] = f"Bearer {token}"

    tmp = dest.with_name(dest.name + ".tmp")
    print(f"  downloading: {dest.name}")
    with httpx.stream("GET", url, headers=headers, follow_redirects=True, timeout=60) as r:
        r.raise_for_status()
        total = int(r.headers.get("content-length", 0)) or None
        with tmp.open("wb") as f, tqdm(total=total, unit="B", unit_scale=True, leave=False) as bar:
            for chunk in r.iter_bytes(chunk_size=1 << 20):
                f.write(chunk)
                bar.update(len(chunk))
    tmp.replace(dest)


# --- Qwen3-Embedding-0.6B -----------------------------------------------------
# Sentence embedding model used by the search indexer. fastembed loads it via
# `UserDefinedEmbeddingModel` from flat files (`model.onnx`, `model.onnx_data`,
# and four tokenizer files), so the directory layout here is a contract with
# `embeddings/embedder.rs`.

QWEN3_MODEL_ID = "Qwen/Qwen3-Embedding-0.6B"
QWEN3_REVISION = "97b0c614be4d77ee51c0cef4e5f07c00f9eb65b3"

# Everything that determines the bytes on disk. A directory whose
# `recipe.json` differs from this is rebuilt rather than skipped, so changing
# the quantization settings here is enough to trigger a re-export.
#
# Parity of this recipe against sentence-transformers was checked once, by
# hand against an fp32 reference, on a right-padded mixed-length batch:
# 0.9986--0.9995 cosine, and batch-independent to within 1e-7. With the
# revision and `exclude-newer` pinned, the export is reproducible; re-check
# when either changes.
QWEN3_RECIPE = {
    "model": QWEN3_MODEL_ID,
    "revision": QWEN3_REVISION,
    "outputs": ["sentence_embedding"],
    # Weight-only int8: activations stay fp32, so an embedding never depends
    # on which other texts shared its batch.
    "quantization": {"bits": 8, "block_size": 128, "symmetric": True, "ops": ["MatMul"]},
    # The 620 MB `embed_tokens` table is quantized by hand, since
    # onnxruntime 1.28 quantizes `Gather` at 4 bits only (and otherwise skips
    # it with nothing but an unconfigured-logger error). See
    # `_quantize_embeddings`.
    "embeddings": {"bits": 8, "symmetric": True, "granularity": "row"},
}


def export_qwen3(dest: Path) -> None:
    recipe_path = dest / "recipe.json"
    if recipe_path.exists() and json.loads(recipe_path.read_text()) == QWEN3_RECIPE:
        print(f"  exists: {dest.name}")
        return

    # Staging is a sibling of `dest` so the final `rename` stays on one
    # filesystem and is atomic.
    staging = dest.with_name(f".{dest.name}.staging")
    exported = staging / "fp32"
    final = staging / "final"
    shutil.rmtree(staging, ignore_errors=True)

    print("  exporting (fp32)")
    _export_fp32(exported)
    print("  pruning token_embeddings output")
    _prune_outputs(exported / "model.onnx", keep=QWEN3_RECIPE["outputs"])
    print("  quantizing (int8 weight-only, int8 embeddings)")
    _quantize(exported / "model.onnx", final / "model.onnx")
    for f in exported.iterdir():
        if not f.name.startswith("model.onnx"):
            shutil.copy2(f, final / f.name)

    (final / "recipe.json").write_text(json.dumps(QWEN3_RECIPE, indent=2) + "\n")
    shutil.rmtree(dest, ignore_errors=True)
    final.rename(dest)
    shutil.rmtree(staging)


def _export_fp32(out: Path) -> None:
    from optimum.exporters.onnx import main_export

    # The Python entry point replaces `optimum-cli`, which `optimum-onnx` no
    # longer ships. optimum's own validation is skipped: its 1e-5 atol on
    # `token_embeddings` flags benign SDPA reordering noise in fp32, and it
    # cannot see the quantized model at all.
    main_export(
        QWEN3_MODEL_ID,
        revision=QWEN3_REVISION,
        output=out,
        task="feature-extraction",
        library_name="sentence_transformers",
        device="cpu",
        do_validation=False,
    )


def _prune_outputs(model_path: Path, keep: list[str]) -> None:
    """Drop graph outputs not in `keep`, editing the graph proto only.

    The sentence-transformers export emits both `token_embeddings`
    (batch x seq x 1024) and the pooled, normalized `sentence_embedding`.
    fastembed's default precedence reaches `sentence_embedding` only because
    no output is named `last_hidden_state`; with a single output it takes the
    `OnlyOne` branch instead, and ORT never materializes the token tensor.
    """
    import onnx

    # `load_external_data=False` reads the 1 MB graph, not the 2.3 GB of
    # weights; saving it back leaves the `model.onnx_data` references intact.
    model = onnx.load(model_path, load_external_data=False)
    outputs = [o for o in model.graph.output if o.name in keep]
    del model.graph.output[:]
    model.graph.output.extend(outputs)
    onnx.save(model, model_path)


def _quantize(src: Path, dest: Path) -> None:
    import onnx
    from onnxruntime.quantization.matmul_nbits_quantizer import (
        DefaultWeightOnlyQuantConfig,
        MatMulNBitsQuantizer,
    )

    q = QWEN3_RECIPE["quantization"]
    config = DefaultWeightOnlyQuantConfig(
        bits=q["bits"],
        block_size=q["block_size"],
        is_symmetric=q["symmetric"],
        op_types_to_quantize=tuple(q["ops"]),
    )
    quantizer = MatMulNBitsQuantizer(onnx.load(src), algo_config=config)
    quantizer.process()
    _quantize_embeddings(quantizer.model.model)

    # The output is under protobuf's 2 GB cap, but `embedder.rs` always reads
    # `model.onnx_data`, so the weights stay external under that exact name.
    # (`ONNXModel.save_model_to_file` would name it `model.onnx.data`.)
    dest.parent.mkdir(parents=True)
    onnx.save_model(
        quantizer.model.model,
        dest,
        save_as_external_data=True,
        all_tensors_to_one_file=True,
        location="model.onnx_data",
    )


def _quantize_embeddings(model) -> None:
    """Replace the fp32 `embed_tokens` lookup with an int8 one, in place.

    Each row (one token's vector) gets its own symmetric scale, so the lookup
    becomes two gathers---int8 rows and their fp32 scales---and a multiply:

        Gather(W, ids)  =>  Cast(Gather(Q, ids), float) * Gather(S, ids)

    `S` is shaped (vocab, 1) so the gathered scales broadcast across the
    hidden dimension. Like the `MatMulNBits` weights, nothing here depends on
    the batch.
    """
    import numpy as np
    from onnx import TensorProto, helper, numpy_helper

    graph = model.graph
    (table,) = [t for t in graph.initializer if t.name.endswith("embed_tokens.weight")]
    (gather,) = [n for n in graph.node if n.op_type == "Gather" and n.input[0] == table.name]
    assert all(a.name != "axis" or a.i == 0 for a in gather.attribute)

    w = numpy_helper.to_array(table)
    scale = np.abs(w).max(axis=1, keepdims=True) / 127
    scale[scale == 0] = 1  # all-zero rows (unused vocab slots) stay zero
    q = np.clip(np.rint(w / scale), -127, 127).astype(np.int8)

    q_name, s_name = f"{table.name}_q8", f"{table.name}_scale"
    ids, out = gather.input[1], gather.output[0]
    graph.initializer.remove(table)
    graph.initializer.extend([
        numpy_helper.from_array(q, q_name),
        numpy_helper.from_array(scale.astype(np.float32), s_name),
    ])

    # Splice the replacement in where the old `Gather` stood, so the node list
    # stays topologically sorted.
    at = list(graph.node).index(gather)
    graph.node.remove(gather)
    for offset, node in enumerate([
        helper.make_node("Gather", [q_name, ids], [f"{out}_q8"]),
        helper.make_node("Cast", [f"{out}_q8"], [f"{out}_f32"], to=TensorProto.FLOAT),
        helper.make_node("Gather", [s_name, ids], [f"{out}_scale"]),
        helper.make_node("Mul", [f"{out}_f32", f"{out}_scale"], [out]),
    ]):
        graph.node.insert(at + offset, node)


# --- Main ---------------------------------------------------------------------


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("models_dir", nargs="?", help="defaults to $MODELS_DIR, then packages/core/models")
    models_dir = resolve_models_dir(parser.parse_args().models_dir)
    models_dir.mkdir(parents=True, exist_ok=True)
    print(f"Models directory: {models_dir}")

    # whisper-rs uses whisper.cpp's GGML format. large-v3 is the full
    # 32-decoder-layer model (~1.5B params)---significantly more accurate
    # than the distilled turbo variant (4 layers) for proper nouns,
    # punctuation, and disfluencies. Multilingual, but we force language="en"
    # at inference time.
    print("\n==> Whisper ggml-large-v3.bin (GGML, ~2.9 GB)")
    download(
        f"{HF_BASE}/ggerganov/whisper.cpp/resolve/main/ggml-large-v3.bin",
        models_dir / "ggml-large-v3.bin",
    )

    # Used by whisper.cpp's built-in VAD to pre-segment audio before ASR.
    # whisper.cpp feeds each detected speech segment to Whisper
    # independently, preventing unrelated speech from merging into one
    # segment. Critical for the Q&A interview pattern (short question / very
    # long answer).
    print("\n==> silero-vad v6.2.0 (GGML, ~2 MB)")
    download(
        f"{HF_BASE}/ggml-org/whisper-vad/resolve/main/ggml-silero-v6.2.0.bin",
        models_dir / "ggml-silero-v6.2.0.bin",
    )

    print("\n==> Qwen3-Embedding-0.6B (ONNX, int8, ~610 MB)")
    export_qwen3(models_dir / "Qwen3-Embedding-0.6B")

    # Speech/silence frame classifier used to detect speaker turn boundaries.
    # This is the ONNX export from the pyannote-rs v0.1.0 release---the
    # upstream pyannote HuggingFace repo only ships a pytorch checkpoint.
    # `transcription/diarize/segmentation.rs` drives it directly through
    # `ort`.
    print("\n==> pyannote-segmentation-3.0 (ONNX, ~6 MB)")
    download(
        "https://github.com/thewh1teagle/pyannote-rs/releases/download/v0.1.0/segmentation-3.0.onnx",
        models_dir / "pyannote-segmentation-3.0.onnx",
    )

    # ECAPA-TDNN 1024 trained on VoxCeleb, from the official WeSpeaker
    # HuggingFace org. Turns a diarized speech segment into a fixed-length
    # embedding for clustering; `transcription/diarize/embedding.rs` drives
    # it directly through `ort`, with `knf-rs` computing the log-mel
    # filterbank features it expects.
    print("\n==> wespeaker voxceleb ECAPA-TDNN 1024 (ONNX, ~59 MB)")
    download(
        f"{HF_BASE}/Wespeaker/wespeaker-voxceleb-ecapa-tdnn1024-LM/resolve/main/voxceleb_ECAPA1024_LM.onnx",
        models_dir / "wespeaker_en_voxceleb_ECAPA1024.onnx",
    )

    print(f"\nDone. All models in {models_dir}")


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        sys.exit(130)
