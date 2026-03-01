# syntax=docker/dockerfile:1.7-labs

# ── Builder ───────────────────────────────────────────────────────────────────
#
# Full Rust image (includes cargo, rustc, C toolchain).  Build-time C/C++
# dependencies:
#   clang / libclang-dev  — bindgen needs libclang to parse whisper.cpp headers
#   cmake                 — whisper.cpp CMake build system
#   libssl-dev / pkg-config — reqwest / hf-hub TLS for model downloads at runtime
FROM rust:1.85-bookworm AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    clang \
    cmake \
    libclang-dev \
    libssl-dev \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# ── Dependency cache layer ────────────────────────────────────────────────────
#
# Copy workspace manifests and a stub src/main.rs so cargo can compile all
# dependencies without the real application source.  This layer is only
# invalidated when Cargo.lock changes, not when src/ changes — so iterative
# builds that only touch Rust source skip the expensive native dependency
# compilations (whisper.cpp, ort runtime, fastembed).
COPY Cargo.toml Cargo.lock ./
COPY packages/auohp-api/Cargo.toml packages/auohp-api/

RUN mkdir -p packages/auohp-api/src \
    && echo 'fn main() {}' > packages/auohp-api/src/main.rs \
    && cargo build --release -p auohp-api \
    && rm packages/auohp-api/src/main.rs

# ── Application source ────────────────────────────────────────────────────────
COPY packages/auohp-api/src packages/auohp-api/src/

# touch forces cargo to recompile the application crate even though the
# deps layer cached the previous stub binary.
RUN touch packages/auohp-api/src/main.rs \
    && cargo build --release -p auohp-api

# ── Collect OnnxRuntime shared library ───────────────────────────────────────
#
# ort-download-binaries fetches libonnxruntime.so at build time and places it
# in the cargo OUT_DIR.  Copy it to a stable path so the runtime stage can
# find it regardless of the cargo-generated path hash.
RUN find target/release -name "libonnxruntime*.so*" -exec cp -P {} /usr/local/lib/ \; \
    && ls /usr/local/lib/libonnxruntime*

# ── Runtime ───────────────────────────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime

# ca-certificates — needed at runtime when fastembed / pyannote-rs download
#   ONNX model weights from Hugging Face Hub on first use.
# libstdc++6 — C++ standard library; whisper-rs links against it.
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libstdc++6 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/auohp-api /usr/local/bin/auohp-api
COPY --from=builder /usr/local/lib/libonnxruntime*.so* /usr/local/lib/

# Register the copied shared library with the dynamic linker.
RUN ldconfig

# ── Model cache volume ────────────────────────────────────────────────────────
#
# HF_HOME redirects the HuggingFace Hub client (used by fastembed and
# pyannote-rs) to store downloaded ONNX weights under /models, which is
# mounted as a named Docker volume.  This means the ~200 MB of model files
# (BGE-small ~33 MB, pyannote segmentation ~17 MB, wespeaker ~130 MB) are
# downloaded once and reused across container restarts and image rebuilds.
ENV HF_HOME=/models

EXPOSE 6060

ENTRYPOINT ["/usr/local/bin/auohp-api"]
