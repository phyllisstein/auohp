# Transcription Pipeline Design for Rust

## Status

Proposed (revised) -- not yet implemented.

## Context

The existing transcription pipeline lives in `packages/scripts/src/subwhisp/` and is pure Python. It chains three stages:

1. **WhisperX** (Whisper large-v3) -- speech-to-text with batch inference.
2. **wav2vec2 forced alignment** -- refines Whisper's coarse segment timestamps to word-level start/end times.
3. **pyannote/speaker-diarization-3.1** -- assigns a speaker label (`SPEAKER_00`, `SPEAKER_01`, ...) to each word.

A fourth stage, **spaCy NLP semantic chunking** (`subber.py`), splits the diarized transcript into caption-sized subtitle segments using grammatical analysis. This stage is being **discarded** in the new pipeline in favor of simpler, time-based chunking.

The Rust service at `packages/auohp-api` runs all inference natively -- no Python runtime, no subprocess. This is required so the service can report granular progress to GraphQL clients during long-running transcription jobs.

---

## Recommended Approach: Native Rust Inference

All ML inference runs inside the Rust process using compiled/ONNX models. No Python dependency at runtime.

### Component Stack

| Stage | Crate | Model | Notes |
|---|---|---|---|
| Audio decoding | `symphonia` | -- | Decode MP4/MP3/WAV to PCM f32 samples. Pure Rust, no ffmpeg. |
| Speech-to-text | `whisper-rs` v0.15+ | `ggml-large-v3` | Bindings to whisper.cpp. Supports GGML-quantized models. |
| Word-level timestamps | whisper.cpp DTW | (same model) | Enable `dtw_token_timestamps` + `token_timestamps` on `FullParams`. |
| Speaker segmentation | `ort` (ONNX Runtime) | `pyannote/segmentation-3.0` | ONNX export available at `onnx-community/pyannote-segmentation-3.0`. 10-second sliding window, outputs per-frame speaker activity. |
| Speaker embedding | `ort` (ONNX Runtime) | `wespeaker-voxceleb-resnet34-LM` | Extracts a 256-dim embedding per speaker segment. |
| Speaker clustering | Manual (cosine similarity + agglomerative) | -- | Cluster embeddings to assign speaker IDs. This is the logic `pyannote-rs` implements. |
| Audio features for embedding | `kaldi-native-fbank` or equivalent | -- | Extract filter bank features (FBank) from PCM for the wespeaker model. `native-pyannote-rs` uses `kaldi-native-fbank` (pure Rust). |

### Why whisper-rs (not candle)

- **whisper-rs** wraps whisper.cpp, the most battle-tested C++ Whisper implementation. It supports GGML quantized models (large-v3 at ~3GB instead of ~6GB fp16), Metal/CoreML acceleration on macOS, and CUDA on Linux.
- **candle** (HuggingFace Rust ML framework) has a Whisper example but it is less mature for production use. It does not implement DTW token timestamps out of the box. candle would require implementing the DTW cross-attention alignment from scratch.
- whisper.cpp's DTW implementation is already proven and accessible through whisper-rs.

### Why ort (not candle) for diarization

- `pyannote-rs` already demonstrates that the pyannote segmentation and wespeaker embedding models run correctly via ONNX Runtime in Rust. The ONNX exports are published on HuggingFace (`onnx-community/pyannote-segmentation-3.0`).
- `ort` is the standard Rust crate for ONNX Runtime (maintained by pyke.io), supports CoreML/CUDA/DirectML execution providers, and is what `pyannote-rs` uses internally.
- candle could theoretically load these models but would require writing custom model architectures rather than using pre-exported ONNX graphs.

### Using pyannote-rs directly vs. rolling our own

`pyannote-rs` (v0.3.x, by thewh1teagle) and its fork `native-pyannote-rs` (by RustedBytes) package the full diarization pipeline:

- Segmentation via `pyannote/segmentation-3.0` ONNX model
- Speaker embedding via `wespeaker-voxceleb-resnet34-LM` ONNX model
- Agglomerative clustering with cosine similarity
- Audio feature extraction via `kaldi-native-fbank` (native-pyannote-rs) or C++ bindings (pyannote-rs)

**Recommendation: use `pyannote-rs` (or `native-pyannote-rs`) as a dependency rather than reimplementing diarization.** The crate handles the non-trivial clustering and VAD logic. If we need to customize (e.g., fixed speaker count for interviews = 2), we can fork or contribute upstream.

Performance: 1 hour of audio diarized in under 1 minute on CPU. CoreML acceleration available on macOS.

---

## Word-Level Timestamps: DTW Approach

### How it works

whisper.cpp implements Dynamic Time Warping (DTW) over the decoder's cross-attention weights to produce per-token timestamps. When enabled:

1. During decoding, whisper.cpp captures the cross-attention weight matrix between decoder tokens and encoder audio frames.
2. DTW aligns each token to its most likely audio frame position.
3. Each token gets a `t_dtw` timestamp (in centiseconds) representing when in the audio it was spoken.

### whisper-rs API

```rust
// On WhisperContextParameters (at model load time):
ctx_params.set_dtw_token_timestamps(true);
ctx_params.set_dtw_aheads_preset(DtwAheadsPreset::LargeV3);

// On FullParams (at inference time):
params.set_token_timestamps(true);

// After inference, extract per-token data:
for seg_idx in 0..state.full_n_segments()? {
    for tok_idx in 0..state.full_n_tokens(seg_idx)? {
        let token_data = state.full_get_token_data(seg_idx, tok_idx)?;
        let text = state.full_get_token_text(seg_idx, tok_idx)?;
        // token_data.t0, token_data.t1 = segment-level times (centiseconds)
        // token_data.t_dtw = DTW-aligned time for this specific token
    }
}
```

Tokens are sub-word units (BPE). Multiple tokens may compose a single word (e.g., `"spec"` + `"ifically"` = `"specifically"`). The pipeline must merge consecutive tokens that form a single word, using the first token's `t_dtw` as the word start and the last token's `t_dtw` (or next token's `t_dtw`) as the word end.

### Accuracy tradeoff vs. wav2vec2 forced alignment

| Method | Mechanism | Typical accuracy |
|---|---|---|
| WhisperX (wav2vec2) | Separate phoneme-level forced alignment model | Best-in-class, ~20ms precision |
| whisper.cpp DTW | Cross-attention weight alignment | Good, ~50-100ms precision |
| whisper.cpp token probabilities (no DTW) | Timestamp token probability after each sub-word | Poor, ~1s precision |

The DTW approach is a meaningful step up from naive token timestamps. For an oral history caption editor where the user is scrubbing through video and matching text to audio, ~50-100ms precision is sufficient. The editor highlights statements, not individual words, so sub-100ms word alignment is adequate.

**This accuracy tradeoff is accepted** in exchange for eliminating the Python runtime dependency and gaining in-process progress reporting.

---

## Diarization: Accuracy Assessment

### pyannote-rs vs. Python pyannote

| Aspect | Python pyannote 3.1 | pyannote-rs (ONNX) |
|---|---|---|
| Segmentation model | segmentation-3.0 (PyTorch) | segmentation-3.0 (ONNX export) |
| Embedding model | pyannote/embedding (PyTorch) | wespeaker-voxceleb-resnet34-LM (ONNX) |
| Clustering | Agglomerative (scipy) | Agglomerative (custom Rust) |
| VAD | Integrated | Integrated |
| Overlap handling | Yes | Limited |

The segmentation model is identical -- same weights, just ONNX-exported. The embedding model differs: pyannote's own embedding model vs. wespeaker. Both are competitive on VoxCeleb benchmarks (wespeaker resnet34-LM achieves ~2.8% EER on VoxCeleb1).

For oral history interviews with typically 2 speakers (interviewer + interviewee) in a controlled recording environment, diarization accuracy should be high with either approach. The challenging cases for diarization -- noisy environments, many overlapping speakers, phone calls -- are not typical of this corpus.

**Assessment: pyannote-rs is production-viable for this use case.** The controlled 2-speaker interview format is the easiest scenario for diarization.

---

## Progress Reporting

A primary motivation for native Rust inference is the ability to report fine-grained progress to GraphQL clients. The transcription pipeline has four observable phases:

### Phase model

```
Phase 1: Audio decoding     (fast, seconds)
Phase 2: Whisper inference   (slow, minutes -- bulk of the time)
Phase 3: Diarization         (moderate, < 1 min for 1 hour of audio)
Phase 4: Statement assembly  (fast, milliseconds)
```

### Progress events

The pipeline emits progress updates via a `tokio::sync::broadcast` channel. Each event is a struct:

```rust
enum TranscriptionPhase {
    AudioDecode,
    Transcription,
    Diarization,
    Assembly,
}

struct TranscriptionProgress {
    job_id: String,
    phase: TranscriptionPhase,
    /// 0.0 to 1.0 within the current phase
    phase_progress: f32,
    /// 0.0 to 1.0 across all phases (weighted)
    overall_progress: f32,
    /// Human-readable status message
    message: String,
}
```

### How each phase reports progress

| Phase | Progress source |
|---|---|
| Audio decoding | Bytes read / total file size |
| Whisper inference | whisper.cpp progress callback (`whisper_full_params.progress_callback`). whisper-rs exposes this as `set_progress_callback()`. The callback receives a percentage (0-100) representing how much of the audio has been processed. |
| Diarization | Sliding window position / total audio duration. The segmentation model processes 10-second windows, so progress = windows_completed / total_windows. |
| Assembly | Instantaneous (too fast to need progress). |

### Phase weighting

For a typical 1-hour interview:
- Audio decode: ~1% of total time
- Whisper inference: ~90% of total time
- Diarization: ~8% of total time
- Assembly: ~1% of total time

The `overall_progress` field applies these weights so the progress bar moves steadily rather than jumping.

### GraphQL subscription

Progress events are exposed via a GraphQL subscription:

```graphql
type Subscription {
    transcriptionProgress(jobId: ID!): TranscriptionProgress!
}

type TranscriptionProgress {
    jobId: ID!
    phase: TranscriptionPhase!
    phaseProgress: Float!
    overallProgress: Float!
    message: String!
}
```

The axum handler bridges the `broadcast` channel to an SSE or WebSocket stream that async-graphql subscriptions can consume.

---

## Chunking Strategy (Replacing spaCy)

The current `subber.py` uses spaCy NLP to split transcripts into caption-sized chunks at grammatically appropriate boundaries (clause breaks, punctuation, noun phrase edges). This is complex (~400 lines), fragile, and a deployment burden (requires spaCy models).

### New strategy: Speaker-change + time-window chunking

Statements are defined by two rules, applied in order:

1. **Speaker change boundary.** Every time the speaker label changes, start a new statement. This is the primary segmentation.

2. **Maximum duration split.** If a single-speaker run exceeds a configurable maximum duration (default: 30 seconds), split at the nearest sentence-ending punctuation (`.`, `?`, `!`) within the word stream. If no sentence boundary exists within the window, split at the nearest word boundary after the max duration.

This is already essentially what `whisper_to_json()` in `whisperer.py` does (merge consecutive same-speaker segments), but the new pipeline makes the max-duration cap explicit.

### Why this is sufficient

- The caption editor displays one statement per block. Long statements are fine -- the editor handles scrolling.
- The search index operates on statement text. Shorter statements mean more granular search hits, but 30-second windows are already granular enough for oral history.
- The complex grammatical splitting was designed for subtitle display (2-line, 42-character captions). The new system does not generate subtitles -- it generates editor statements.

### VTT generation

If VTT/subtitle output is needed in the future, it can be generated as a separate pass over the word-level data, splitting at fixed character widths. This does not need to happen at transcription time.

---

## Proposed Output JSON Schema

This is the canonical format produced by the transcription pipeline and consumed by the Neo4j seed layer. The schema is unchanged from the earlier revision -- the native Rust pipeline produces the same output format.

### Top-level structure

```json
{
  "version": "2.0",
  "source": {
    "file": "025_lei_chou.mp4",
    "duration_seconds": 5765.42,
    "pipeline": "auohp-native-1.0",
    "model": "ggml-large-v3",
    "diarization_model": "pyannote/segmentation-3.0 + wespeaker-voxceleb-resnet34-LM",
    "alignment_method": "whisper-dtw",
    "created_at": "2026-02-28T12:00:00Z"
  },
  "speakers": [
    { "id": "SPEAKER_00", "label": null },
    { "id": "SPEAKER_01", "label": null }
  ],
  "statements": [
    {
      "speaker": "SPEAKER_00",
      "start_time": 0.0,
      "end_time": 12.45,
      "text": "So I came to New York in 1985, and the first thing I remember is the energy.",
      "words": [
        { "word": "So", "start": 0.0, "end": 0.15, "speaker": "SPEAKER_00" },
        { "word": "I", "start": 0.18, "end": 0.22, "speaker": "SPEAKER_00" },
        { "word": "came", "start": 0.25, "end": 0.48, "speaker": "SPEAKER_00" },
        { "word": "to", "start": 0.50, "end": 0.58, "speaker": "SPEAKER_00" },
        { "word": "New", "start": 0.61, "end": 0.75, "speaker": "SPEAKER_00" },
        { "word": "York", "start": 0.76, "end": 1.02, "speaker": "SPEAKER_00" },
        { "word": "in", "start": 1.10, "end": 1.18, "speaker": "SPEAKER_00" },
        { "word": "1985,", "start": 1.22, "end": 1.68, "speaker": "SPEAKER_00" },
        { "word": "and", "start": 1.85, "end": 1.95, "speaker": "SPEAKER_00" },
        { "word": "the", "start": 1.98, "end": 2.08, "speaker": "SPEAKER_00" },
        { "word": "first", "start": 2.10, "end": 2.38, "speaker": "SPEAKER_00" },
        { "word": "thing", "start": 2.40, "end": 2.60, "speaker": "SPEAKER_00" },
        { "word": "I", "start": 2.65, "end": 2.70, "speaker": "SPEAKER_00" },
        { "word": "remember", "start": 2.72, "end": 3.15, "speaker": "SPEAKER_00" },
        { "word": "is", "start": 3.20, "end": 3.30, "speaker": "SPEAKER_00" },
        { "word": "the", "start": 3.35, "end": 3.42, "speaker": "SPEAKER_00" },
        { "word": "energy.", "start": 3.45, "end": 3.88, "speaker": "SPEAKER_00" }
      ]
    },
    {
      "speaker": "SPEAKER_01",
      "start_time": 4.10,
      "end_time": 8.92,
      "text": "And what brought you to ACT UP specifically?",
      "words": [
        { "word": "And", "start": 4.10, "end": 4.22, "speaker": "SPEAKER_01" },
        { "word": "what", "start": 4.25, "end": 4.40, "speaker": "SPEAKER_01" },
        { "word": "brought", "start": 4.42, "end": 4.68, "speaker": "SPEAKER_01" },
        { "word": "you", "start": 4.70, "end": 4.82, "speaker": "SPEAKER_01" },
        { "word": "to", "start": 4.85, "end": 4.92, "speaker": "SPEAKER_01" },
        { "word": "ACT", "start": 5.00, "end": 5.20, "speaker": "SPEAKER_01" },
        { "word": "UP", "start": 5.22, "end": 5.45, "speaker": "SPEAKER_01" },
        { "word": "specifically?", "start": 5.50, "end": 6.10, "speaker": "SPEAKER_01" }
      ]
    }
  ]
}
```

### Field definitions

| Field | Type | Description |
|---|---|---|
| `version` | string | Schema version. `"2.0"` for this format. |
| `source.file` | string | Original input filename. |
| `source.duration_seconds` | float | Total audio duration in seconds. |
| `source.pipeline` | string | Pipeline identifier (`"auohp-native-1.0"` for the Rust pipeline). |
| `source.model` | string | Whisper model used. |
| `source.diarization_model` | string | Diarization models used. |
| `source.alignment_method` | string | `"whisper-dtw"` for native DTW, or `"wav2vec2"` if forced alignment were used. |
| `source.created_at` | string | ISO 8601 timestamp of transcription. |
| `speakers[]` | array | List of detected speakers. |
| `speakers[].id` | string | Machine-assigned speaker ID (e.g. `SPEAKER_00`). |
| `speakers[].label` | string or null | Human-assigned label (e.g. `"Lei Chou"`), null until mapped. |
| `statements[]` | array | Ordered list of transcript statements. |
| `statements[].speaker` | string | Speaker ID for this statement. |
| `statements[].start_time` | float | Start time in seconds from audio start. |
| `statements[].end_time` | float | End time in seconds from audio start. |
| `statements[].text` | string | Full text of the statement. |
| `statements[].words[]` | array | Per-word timing and speaker data. |
| `statements[].words[].word` | string | The word (including trailing punctuation). |
| `statements[].words[].start` | float | Word start time in seconds. |
| `statements[].words[].end` | float | Word end time in seconds. |
| `statements[].words[].speaker` | string | Speaker ID for this word. |

### Design notes

- **Times are floating-point seconds**, not timestamps. The `format_timestamp()` conversion to `MM:SS` or `HH:MM:SS.mmm` strings happens at the presentation layer, not in the canonical data.
- **Speaker labels are opaque IDs.** The `speakers[]` array provides a place to map them to human names, which the caption editor or a seed script can fill in.
- **No `children` array.** The v1 format included `children: [{text: "..."}]` for Slate.js compatibility. The Slate editor shape is a presentation concern and should be constructed by the editor from `text`, not stored in the canonical transcript.
- **No `startTimestamp`/`endTimestamp` strings.** Formatted timestamps are derived from `start_time`/`end_time` at the point of use.
- **Words include speaker.** This supports future per-word speaker correction and enables detecting mid-statement speaker overlaps.
- **`alignment_method` field.** Distinguishes DTW-based timestamps from wav2vec2 forced alignment, so downstream consumers can adjust confidence expectations.

---

## What to Keep vs. Discard from subwhisp

The entire Python `subwhisp` package is superseded by the native Rust pipeline. Nothing is called at runtime.

### Reference value (logic to port to Rust)

| Component | File | What to port |
|---|---|---|
| Speaker-merging logic | `whisperer.py` `whisper_to_json()` | The pattern of merging consecutive same-speaker segments. Reimplement in Rust. |
| Segment structure | `whisperer.py` lines 41-68 | The field names and nesting conventions inform the v2.0 schema. |

### Discard entirely

| Component | File | Reason |
|---|---|---|
| WhisperX Python wrapper | `whisperer.py` | Replaced by `whisper-rs`. |
| wav2vec2 alignment | `whisperer.py` `align_transcription()` | Replaced by whisper.cpp DTW timestamps. |
| pyannote Python wrapper | `whisperer.py` `diarize_audio_file()` | Replaced by `pyannote-rs` / `ort`. |
| spaCy NLP chunking | `subber.py` (all of it) | Replaced by speaker-change + max-duration splitting in Rust. |
| `format_timestamp()` | `subber.py` | Timestamps stored as floats; formatting at presentation layer. |
| VTT generation | `subber.py` `write_vtt()` | Not needed at transcription time. |
| Caption JSON generation | `subber.py` `write_json()` / `to_json_captions()` | Replaced by v2.0 schema. |
| CLI entry point | `cli.py` | Replaced by GraphQL mutations on the Rust service. |
| Model download | `cli.py` `models` command | Replaced by a Rust model-download utility or manual setup. |

---

## Pipeline Architecture

```
                         Rust process (auohp-api)
                        +---------------------------------------------+
                        |                                             |
[GraphQL mutation] ---->| 1. Audio decode (symphonia)                 |
                        |    -> PCM f32 samples                       |
                        |    -> progress: bytes_read / file_size      |
                        |                                             |
                        | 2. Whisper inference (whisper-rs)            |
                        |    -> segments with DTW token timestamps     |
                        |    -> progress: whisper progress_callback   |
                        |                                             |
                        | 3. Token-to-word merging                    |
                        |    -> merge BPE sub-tokens into words       |
                        |    -> word start = first token t_dtw        |
                        |    -> word end = next token t_dtw           |
                        |                                             |
                        | 4. Diarization (pyannote-rs / ort)          |
                        |    -> speaker segments                      |
                        |    -> assign speaker to each word           |
                        |    -> progress: window_pos / total_duration |
                        |                                             |
                        | 5. Statement assembly                       |
                        |    -> merge same-speaker word runs           |
                        |    -> split at speaker changes               |
                        |    -> split at max duration (30s)            |
                        |    -> produce v2.0 JSON                     |
                        |                                             |
                        | 6. Neo4j seeding                            |
                        |    -> Cypher UNWIND batch writes             |
                        +---------------------------------------------+
                              |
                              | broadcast channel
                              v
                        [GraphQL subscription: transcriptionProgress]
```

### Threading model

Whisper inference and diarization are CPU/GPU-bound. They must run on a dedicated thread pool (`tokio::task::spawn_blocking` or `rayon`) to avoid blocking the async runtime. The progress callback bridges from the blocking thread to the async broadcast channel.

```rust
// Sketch (not implementation):
let (tx, _) = broadcast::channel::<TranscriptionProgress>(64);

let tx_whisper = tx.clone();
tokio::task::spawn_blocking(move || {
    let params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    params.set_token_timestamps(true);
    params.set_progress_callback(move |progress| {
        let _ = tx_whisper.send(TranscriptionProgress {
            phase: TranscriptionPhase::Transcription,
            phase_progress: progress as f32 / 100.0,
            // ...
        });
    });
    state.full(params, &audio_data)?;
    // extract tokens...
});
```

---

## Cargo Dependencies (Additions)

```toml
# In packages/auohp-api/Cargo.toml

# Whisper speech-to-text (bindings to whisper.cpp)
whisper-rs = "0.15"

# ONNX Runtime for diarization models
ort = { version = "2", features = ["load-dynamic"] }

# Speaker diarization (pyannote segmentation + wespeaker via ONNX)
# Evaluate: pyannote-rs or native-pyannote-rs
pyannote-rs = "0.3"

# Audio decoding (MP4/MP3/WAV -> PCM)
symphonia = { version = "0.5", features = ["mp3", "isomp4", "aac", "pcm"] }

# Serialization for the v2.0 JSON schema
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

---

## Open Questions

1. **Speaker mapping UI.** The v2.0 schema includes `speakers[].label = null`. When and how does the user map `SPEAKER_00` to `"Lei Chou"`? This likely happens in the caption editor after the initial seed.

2. **Incremental re-transcription.** If a user re-runs transcription on the same interview, should the seed layer merge or replace? The current design assumes replace (delete existing nodes, re-seed).

3. **Model distribution.** The GGML Whisper model (~3GB), segmentation ONNX (~17MB), and wespeaker ONNX (~130MB) need to be downloaded before first use. Options: (a) a `auohp-api models download` CLI subcommand, (b) lazy download on first transcription, (c) Docker image with models baked in.

4. **`pyannote-rs` vs. `native-pyannote-rs`.** The `native-pyannote-rs` fork replaces C++ audio feature extraction with pure Rust (`kaldi-native-fbank`), simplifying the build. Need to verify it produces equivalent results. Prefer `native-pyannote-rs` if so.

5. **BPE token merging heuristics.** whisper.cpp BPE tokens need to be merged into words. The standard heuristic: tokens starting with a space begin a new word; tokens without a leading space continue the previous word. Edge cases (punctuation, numbers, non-ASCII) need testing against the oral history corpus.

6. **Fixed speaker count.** Oral history interviews have exactly 2 speakers (interviewer + interviewee). Passing `num_speakers=2` to the diarization clustering step should improve accuracy. Need to verify `pyannote-rs` supports this parameter.
