# AUOHP Desktop App — Design Document

**Status:** Draft — not yet started
**Last updated:** 2026-03-09

## 1. Goal

Distribute the AUOHP transcription pipeline (Whisper ASR + pyannote diarization + fastembed embeddings) as a **macOS menu bar application** so that non-technical users can run local GPU-accelerated inference without installing Rust, Python, or ONNX toolchains. The app runs in the background, communicates with the cloud-hosted webapp over localhost HTTP/WebSocket, and reports progress through system tray UI.

## 2. Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│  Tauri process (single binary)                                  │
│                                                                 │
│  ┌───────────────┐   ┌──────────────────────────────────────┐   │
│  │ System tray   │   │  Embedded axum server (:34042)       │   │
│  │ (idle/busy/   │◄──│  GET  /health                        │   │
│  │  done icons)  │   │  POST /transcribe  (multipart)       │   │
│  │               │   │  GET  /ws/{jobId}  (WebSocket)       │   │
│  │ Status popover│   │  POST /cancel/{jobId}                │   │
│  └───────────────┘   └──────────────┬───────────────────────┘   │
│                                     │                           │
│  ┌──────────────────────────────────▼───────────────────────┐   │
│  │  auohp-api library crate                                 │   │
│  │  audio::decode_file → whisper::transcribe                │   │
│  │  → diarize::diarize → merge → TranscriptionResult        │   │
│  │  embeddings::Embedder                                    │   │
│  └──────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
        ▲ http://localhost:34042
        │ (secure-context exemption: Chrome 94+, Firefox, Safari 15.4+)
        │
┌───────┴──────────────────────────────┐
│  Browser: cloud-hosted webapp (HTTPS)│
│  fetch('/health') on page load       │
│  Shows "local GPU available" badge   │
│  Uploads files, receives WS progress │
└──────────────────────────────────────┘
```

## 3. Crate Restructuring (prerequisite)

The transcription pipeline currently lives in `packages/auohp-api/src/transcription/` and is shared with the `transcribe` binary via a `#[path]` hack. This doesn't work across crate boundaries.

### Target layout

```
packages/auohp-api/
├── Cargo.toml          # workspace member
├── src/
│   ├── lib.rs          # NEW — re-exports transcription + embeddings as library
│   ├── main.rs         # axum GraphQL server (unchanged behavior)
│   └── graphql/        # (unchanged)
│
│   # These modules move into lib.rs's module tree:
│   ├── embeddings.rs
│   ├── neo4j.rs
│   └── transcription/
│       ├── mod.rs
│       ├── audio.rs
│       ├── diarize.rs
│       ├── pipeline.rs
│       ├── types.rs
│       └── whisper.rs
│
├── src/bin/
│   └── transcribe.rs   # remove #[path] hack, use `auohp_api::transcription::*`
│
packages/auohp-desktop/
├── Cargo.toml          # depends on auohp-api (lib), tauri
├── tauri.conf.json
├── src/
│   ├── main.rs         # Tauri entry point + embedded axum
│   ├── tray.rs         # System tray setup and state machine
│   ├── server.rs       # axum routes: /health, /transcribe, /ws, /cancel
│   └── jobs.rs         # Job queue, progress channel, cancellation
├── ui/                 # Small HTML/CSS status popover (built by Vite or plain)
│   ├── index.html
│   └── status.js
└── icons/
    ├── tray-idle.png
    ├── tray-busy.png
    └── tray-done.png
```

### Why `lib.rs` in `auohp-api` instead of a separate `auohp-core` crate

The transcription code is tightly coupled to the existing feature flags (`metal`, `cuda`) and dependency pins (`ort = "=2.0.0-rc.10"`, `fastembed ~5.8`). Extracting to a separate crate would mean duplicating those constraints. Adding a `[lib]` section to the existing `Cargo.toml` is simpler — both `main.rs` and `transcribe.rs` become `[[bin]]` targets that depend on the crate's own library, and `auohp-desktop` depends on `auohp-api` as a path dependency with `default-features = false`.

```toml
# In packages/auohp-api/Cargo.toml — add:
[lib]
name = "auohp_api"
path = "src/lib.rs"

[[bin]]
name = "auohp-api"
path = "src/main.rs"

[[bin]]
name = "transcribe"
path = "src/bin/transcribe.rs"
```

```toml
# In packages/auohp-desktop/Cargo.toml:
[dependencies]
auohp-api = { path = "../auohp-api", default-features = false }
tauri = { version = "2", features = ["tray-icon"] }
# ...
```

### Module visibility changes

Currently `transcription` and `embeddings` are `mod` (private). For `lib.rs`:

```rust
// src/lib.rs
pub mod embeddings;
pub mod transcription;
pub mod neo4j;
```

`main.rs` and `transcribe.rs` then use `use auohp_api::transcription::{run, PipelineConfig};` etc.

## 4. Tauri App Structure

### 4.1 Entry point

```rust
// packages/auohp-desktop/src/main.rs
fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            // Build the system tray
            tray::setup(app.handle())?;

            // Spawn the embedded axum server on the Tauri async runtime.
            // AppHandle is Clone + Send + 'static — designed for this.
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(server::run(handle));

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("failed to run app");
}
```

**Why this works:** Tauri v2 uses tokio under the hood (`tauri::async_runtime` is tokio). Spawning axum as a task means the HTTP server, inference futures, and Tauri's event loop share one thread pool. No IPC overhead, no separate process.

### 4.2 System tray states

| State | Icon | Menu items |
|---|---|---|
| Idle | `tray-idle.png` | "AUOHP Transcriber", separator, "Start at Login", "Quit" |
| Transcribing | `tray-busy.png` (animated) | "Transcribing: 34%...", "Cancel", separator, "Quit" |
| Complete | `tray-done.png` | "Transcription complete", separator, "Start at Login", "Quit" |
| Error | `tray-idle.png` + notification | "Last job failed: {reason}", separator, "Quit" |

Tray updates are driven by `AppHandle::emit("progress", payload)` events from the job runner, listened to by the tray module.

### 4.3 Status popover

A hidden-by-default Tauri window (~320x400px) that shows:
- Current job stage and progress bar
- Live transcript segments as they arrive
- Model download progress (first run)
- Error details

Rendered from a small HTML/JS bundle — not the full webapp.

## 5. HTTP API (embedded axum)

### `GET /health`

```json
{ "status": "ok", "version": "0.1.0", "gpu": "metal" }
```

The webapp calls this on page load to detect local inference availability.

### `POST /transcribe`

Multipart form upload. Fields: `file` (audio/video binary), `maxSpeakers` (optional integer, default 10).

Returns:

```json
{ "jobId": "abc123" }
```

The file is written to a temp directory. Transcription begins immediately on a `spawn_blocking` task.

### `GET /ws/{jobId}`

WebSocket connection. Server pushes progress events:

```jsonc
// Whisper progress callback (0-100%, fired by whisper.cpp internally)
{ "type": "stage", "stage": "transcribing", "percent": 54 }

// Live transcript segments (fired by Whisper's segment callback as decoded)
{ "type": "segment", "speaker": null, "text": "So we marched down to City Hall...", "start": 12.3, "end": 18.7 }

// Diarization progress (per-segment from the outer loop)
{ "type": "stage", "stage": "diarizing", "percent": 45, "segment": 120, "total": 267, "eta_seconds": 82 }

// Other stage transitions
{ "type": "stage", "stage": "decoding", "percent": 0 }
{ "type": "stage", "stage": "embedding", "percent": 90 }

// Model download progress (first run only)
{ "type": "download", "model": "whisper-large-v3-turbo-q8", "percent": 34, "bytes_total": 916455424 }

// Terminal states
{ "type": "complete", "jobId": "abc123" }
{ "type": "error", "message": "unsupported audio codec", "stage": "decoding" }
```

Note: `segment` events during transcription have `"speaker": null` because diarization hasn't run yet. The final result on disk has speaker labels assigned.

### `GET /result/{jobId}`

Returns the completed `TranscriptionResult` JSON from local storage. Returns `404` if the job doesn't exist or `409` if still in progress.

### `POST /cancel/{jobId}`

Signals cancellation. Whisper's progress callback can check a `CancellationToken` and return early. Between pipeline stages (decode → transcribe → diarize → embed), the token is checked explicitly. Diarization checks per-segment.

Returns `204 No Content` on success, `404` if the job doesn't exist.

### CORS

```rust
use tower_http::cors::{CorsLayer, AllowOrigin};

let cors = CorsLayer::new()
    .allow_origin(AllowOrigin::exact("https://auohp.here".parse().unwrap()))
    .allow_methods([Method::GET, Method::POST])
    .allow_headers([header::CONTENT_TYPE]);
```

The browser makes cross-origin requests from `https://auohp.here` to `http://localhost:34042`. The localhost secure-context exemption means this is not mixed-content-blocked, but CORS preflight still fires — `CorsLayer` handles the `OPTIONS` response.

### Port conflict handling

Try ports 34042–34051, bind to the first available. The webapp discovers the service by trying ports in sequence:

```js
async function findLocalService() {
  for (const port of [34042, 34043, 34044]) {
    try {
      const res = await fetch(`http://localhost:${port}/health`, {
        signal: AbortSignal.timeout(500)
      });
      if (res.ok) return port;
    } catch { continue; }
  }
  return null;
}
```

## 6. Local Storage

The desktop app persists job results to disk. Persistence, editing, and search remain remote (Neo4j + cloud API). The local store provides resilience (browser can close mid-transcription) and job history.

```
~/.auohp/jobs/
  abc123/
    input.mp4          # or symlink to original
    result.json        # TranscriptionResult, written atomically on completion
    meta.json          # { status, createdAt, fileName, duration, speakers }
```

- `GET /result/{jobId}` reads `result.json` from disk.
- The status popover lists past jobs by scanning `meta.json` files.
- No database, no complexity.

After the webapp fetches a completed result, it POSTs it to the cloud API for Neo4j seeding. This keeps Neo4j credentials server-side.

## 7. Job Queue and Concurrency

Whisper + diarization saturate all CPU cores (and GPU, with Metal). Running two jobs concurrently would degrade both.

- **Single active job.** A second `POST /transcribe` while one is running returns `409 Conflict` with `{ "activeJobId": "..." }`.
- **No queue.** Users retry manually.
- **State machine:** `Idle → Decoding → Transcribing → Diarizing → Embedding → Complete | Error`

```rust
struct JobState {
    current: Option<Job>,
    cancel_token: CancellationToken,
}

struct Job {
    id: String,
    stage: Stage,
    percent: f32,
    tx: broadcast::Sender<ProgressEvent>,
}
```

`JobState` is wrapped in `Arc<Mutex<JobState>>` and injected as axum state. The inference task sends progress through the `broadcast::Sender`; the WebSocket handler subscribes via `broadcast::Receiver`.

## 8. Progress Reporting from the Pipeline

### Whisper (fine-grained, callback-driven)

whisper-rs exposes two safe callback hooks on `FullParams`:

- **`set_progress_callback_safe`** — `FnMut(i32)`, fires with 0–100% progress as Whisper decodes.
- **`set_segment_callback_safe_lossy`** — fires per segment with `{ segment_index, start_timestamp, end_timestamp, text }`, enabling live transcript streaming to the browser.

Both accept `FnMut` closures, so they can capture and mutate a `broadcast::Sender` directly — no additional synchronization needed.

```rust
pub fn transcribe(
    ctx: &WhisperContext,
    audio: &[f32],
    on_progress: impl FnMut(i32) + 'static,
    on_segment: impl FnMut(SegmentCallbackData) + 'static,
) -> Result<Vec<WhisperSegment>> {
    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 5 });
    params.set_progress_callback_safe(on_progress);
    params.set_segment_callback_safe_lossy(on_segment);
    // ...
}
```

### Diarization (per-segment, from outer loop)

The diarization loop in `diarize.rs` already computes `(current_segment, total_segments)` and logs via `tracing::debug!`. Add a callback parameter:

```rust
pub fn diarize(
    // ...existing params...
    on_progress: impl Fn(usize, usize),  // (current, total)
) -> Result<Vec<DiarizedSegment>> {
    for (i, seg) in raw_segments.iter().enumerate() {
        on_progress(i, total);
        // ...existing logic...
    }
}
```

### Pipeline orchestration

The top-level `pipeline::run` threads callbacks through to each stage:

```rust
pub fn run(
    config: &PipelineConfig,
    input_path: &Path,
    on_progress: impl Fn(PipelineEvent),
) -> Result<TranscriptionResult> { ... }
```

The standalone `transcribe` CLI constructs a callback that prints to stderr. The Tauri app constructs one that captures a `broadcast::Sender<ProgressEvent>`.

## 9. ML Toolchain

### Model choices and rationale

| Component | Model | Params | Dims | Why |
|---|---|---|---|---|
| **ASR** | Whisper `large-v3-turbo` (q8) | 809M | — | Distilled from `large-v3`: 4 decoder layers instead of 32. Faster than `medium.en` on GPU with better accuracy (~4.4% vs ~5.0% WER). `DtwModelPreset::LargeV3Turbo` exists in whisper-rs 0.15. |
| **Diarization** | pyannote segmentation-3.0 + wespeaker | — | — | Still best-in-class open-source diarization. The main alternative (NeMo MSDD) is PyTorch-only with no ONNX export — not viable for single-binary distribution. |
| **Embeddings** | nomic-embed-text-v1.5 | — | 768 | Higher MTEB score than BGE-small-en-v1.5 (65.2 vs 62.2). Available in fastembed as `EmbeddingModel::NomicEmbedTextV15`. Doubles the vector index dimensions (384 → 768) which increases Neo4j storage slightly but improves search quality. |

### Why Rust, not Python

The Python ML ecosystem's advantage is in training and experimentation. At inference time, both languages call the same C++ backends:

- `whisper-rs` → whisper.cpp (C++)
- `pyannote-rs` → ONNX Runtime (C++)
- `fastembed` → ONNX Runtime (C++)

Python's `faster-whisper`, `pyannote.audio`, and `sentence-transformers` call the same engines. The difference is distribution: shipping Python to non-technical users means bundling a conda/venv (~500 MB–1 GB, fragile), fighting PyInstaller/Nuitka (notorious for breaking macOS code signing), or running Docker (wrong abstraction for a menu bar app). Rust + Tauri produces a single signed `.dmg`. For nonprofits with limited IT support, this is decisive.

Python would win if models needed frequent swapping or lacked ONNX/GGML exports. AUOHP's pipeline is stable.

### Model files and sizes

| Model | Size | Download mechanism |
|---|---|---|
| `ggml-large-v3-turbo-q8_0.bin` (Whisper) | ~874 MB | `hf-hub` crate, first run |
| `pyannote-segmentation-3.0.onnx` | ~17 MB | Bundle in `.dmg` or download |
| `wespeaker_en_voxceleb_CAM++.onnx` | ~80 MB | Bundle in `.dmg` or download |
| nomic-embed-text-v1.5 (fastembed) | ~270 MB | Automatic via `fastembed` |

### First-run flow

1. App launches → checks `~/.auohp/models/` for expected files.
2. Missing models trigger the download screen (status popover opens automatically).
3. Download progress events are emitted on any active WebSocket and displayed in the popover.
4. Pyannote models (~97 MB combined) could be bundled in the `.dmg` to reduce first-run friction. Whisper model is too large to bundle.

### Storage management

Status popover shows total model storage and offers "Delete Models" to clear `~/.auohp/models/` and the fastembed HF cache.

## 10. macOS Distribution

### Requirements

- Apple Developer account ($99/yr) for Developer ID Application certificate
- App-specific password for `notarytool`
- Tauri v2 CLI handles `.dmg` packaging

### Build pipeline

```
cargo tauri build --features metal
  → compiles Rust with Metal + CoreML
  → bundles the webview UI
  → .app bundle → codesign → notarize → .dmg
```

Automate via GitHub Actions with `tauri-apps/tauri-action@v0`. Secrets: `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID`.

### Entitlements

```xml
<dict>
    <key>com.apple.security.network.server</key>    <!-- localhost listener -->
    <true/>
    <key>com.apple.security.network.client</key>    <!-- model downloads -->
    <true/>
    <key>com.apple.security.files.user-selected.read-only</key>
    <true/>
</dict>
```

### Auto-update

`tauri-plugin-updater` pointed at a GitHub Releases JSON manifest.

## 11. Scaling Out: Cross-Platform and Server Fallback

The initial design targets macOS Apple Silicon with a small user base (~tens of people). This section considers what changes if the code is released as FOSS for other nonprofits and researchers.

### 11.1 Cross-platform GPU acceleration

The architecture — Tauri menu bar app, embedded axum, localhost WebSocket — is fully cross-platform. The complications are in the inference stack.

**Whisper:** `whisper-rs` wraps `whisper.cpp`, which has backends for Metal, CUDA, OpenCL, and Vulkan. On Windows, the `cuda` feature flag covers Nvidia GPUs. For AMD and Intel GPUs, whisper.cpp has Vulkan support but `whisper-rs` doesn't expose it yet (as of 0.15). CPU fallback always works.

**ONNX Runtime (pyannote + fastembed):** `ort` supports DirectML on Windows as a first-class Execution Provider. DirectML is vendor-agnostic — it runs on any GPU with a DirectX 12 driver (Nvidia, AMD, Intel integrated). A new feature flag would cover it:

```toml
directml = ["ort/directml"]
```

**Platform matrix:**

| Platform | Whisper backend | ONNX EP | Feature flags | Notes |
|---|---|---|---|---|
| macOS Apple Silicon | Metal | CoreML | `metal` | Full acceleration |
| macOS Intel | CPU | CPU | (none) | ~4× slower than Metal |
| Windows + Nvidia | CUDA | CUDA | `cuda` | Full acceleration |
| Windows + AMD/Intel | CPU | DirectML | `directml` | Lopsided: diarization/embedding on GPU, Whisper on CPU |
| Linux + Nvidia | CUDA | CUDA | `cuda` | Full acceleration |
| Linux (no GPU) | CPU | CPU | (none) | Slowest path |

The lopsided Windows AMD/Intel case is notable: whisper.cpp and ONNX Runtime are separate inference engines with separate hardware abstraction layers. Metal and CoreML happen to cover the same Apple hardware, so macOS gets uniform acceleration. On Windows there is no single backend that covers both — CUDA works for both but only on Nvidia, and DirectML only covers ONNX. This is a fundamental fragmentation in the ML inference ecosystem, not something to engineer around.

### 11.2 Server-side fallback

The webapp already talks to the local service over HTTP. If the same API contract (`POST /transcribe`, `GET /ws/{jobId}`, `GET /result/{jobId}`) is deployed behind a cloud endpoint, the webapp doesn't care where inference runs.

```
User has local app  →  fetch('http://localhost:34042/health')  →  200  →  local
User has no local app  →  fetch fails  →  fallback to https://api.auohp.here/transcribe
```

The server-side version is the same Rust binary deployed on a GPU instance, with these differences:

| Concern | Local | Server |
|---|---|---|
| Authentication | None (localhost) | OAuth / API key |
| File upload | Fast (loopback) | Slow (network) — needs resumable upload (tus protocol) |
| Concurrency | Single job, single user | Job queue with worker pool |
| GPU | User's hardware | Provisioned instances (T4/A10G) |
| Cost | Free (user's electricity) | ~$0.50–1.50/GPU-hour |
| Storage | `~/.auohp/jobs/` on disk | S3/R2 bucket with TTL |
| Progress | WebSocket via in-process `broadcast::Sender` | Pub/sub (Redis) to fan out across workers |

### 11.3 What changes for the server deployment

**Job queue:** Locally, a second `POST /transcribe` returns `409 Conflict`. On the server, jobs are queued:

```jsonc
// POST /transcribe → 202 Accepted
{ "jobId": "abc123", "position": 3 }

// WebSocket events include queue status
{ "type": "queued", "position": 3 }
{ "type": "queued", "position": 2 }
{ "type": "stage",  "stage": "decoding", "percent": 0 }
```

A persistent queue (Redis, PostgreSQL, or an in-memory `VecDeque` for simple deployments) replaces the single-job `Arc<Mutex<JobState>>`.

**WebSocket fan-out:** In the local app, the worker running inference and the server handling the WebSocket are the same process, so `broadcast::Sender` works directly. In a multi-worker server deployment, the inference worker and the WebSocket server may be different processes. Redis pub/sub (or SSE as a simpler alternative to WebSocket) bridges the gap.

**Resumable uploads:** Large interview files (multi-GB) over the network need resumable upload support. The tus protocol is the standard approach; `axum-tus` or a custom implementation backed by S3 multipart upload.

**Authentication:** The local service relies on localhost for trust. The server needs real auth — OAuth tokens from the webapp, or API keys for programmatic access.

**Cost management for FOSS/nonprofit use:**
- **Scale-to-zero:** Spin up a GPU instance per job, shut down when idle. Cold start ~2 min on AWS ECS, acceptable for a 5–8 min job.
- **Spot instances:** T4 spot on AWS is ~$0.15/hr vs. $0.53/hr on-demand. Transcription is fault-tolerant (just retry on preemption).
- **Local-first default:** The server fallback exists for users without capable hardware. The FOSS README would say "install the desktop app for free local inference, or deploy the server component for shared use."

### 11.4 What doesn't change

The `lib.rs` extraction (section 3) already makes the pipeline consumable by any binary. The local and server deployments share the same library crate, the same HTTP API contract, and the same progress event schema. The webapp's only decision is which origin to call:

```typescript
const inferenceOrigin = localPort
  ? `http://localhost:${localPort}`
  : 'https://api.auohp.here';
```

This is a "local-first with cloud escape hatch" pattern: the architecture is designed around the local case (simpler, cheaper, faster), and the cloud deployment slots in by adding layers (auth, queue, storage) rather than requiring a redesign.

## 12. Webapp Integration

### Detection

```typescript
const localPort = await findLocalService(); // tries 34042-34044
if (localPort) {
  showLocalGpuBadge();
  enableLocalTranscription(localPort);
}
```

### Upload + progress

```typescript
async function transcribeLocally(file: File, port: number) {
  const form = new FormData();
  form.append('file', file);
  const { jobId } = await fetch(`http://localhost:${port}/transcribe`, {
    method: 'POST', body: form,
  }).then(r => r.json());

  const ws = new WebSocket(`ws://localhost:${port}/ws/${jobId}`);
  ws.onmessage = (e) => {
    const event = JSON.parse(e.data);
    switch (event.type) {
      case 'stage':    updateProgressBar(event.stage, event.percent); break;
      case 'segment':  appendLiveTranscript(event); break;
      case 'complete': fetchResult(jobId, port); break;
      case 'error':    showError(event.message); break;
    }
  };
}
```

### Result handoff

On `complete`, the webapp fetches `GET /result/{jobId}` from the local service (or server), then POSTs it to the cloud API for Neo4j seeding. Credentials stay server-side.

## 13. Implementation Order

| Step | Description | Depends on |
|---|---|---|
| **0** | Add `[lib]` to `auohp-api/Cargo.toml`, create `lib.rs`, make modules `pub`. Remove `#[path]` hack. Verify both binaries compile. | — |
| **1** | Add progress/segment callbacks to `whisper::transcribe`, `diarize::diarize`, and `pipeline::run`. Update `transcribe` CLI to print progress to stderr. | 0 |
| **2** | Scaffold `packages/auohp-desktop` with Tauri v2. System tray with idle state. Hidden popover window. No server yet. | 0 |
| **3** | Embed axum in Tauri. Implement `GET /health`. Verify webapp detection. | 2 |
| **4** | Implement `POST /transcribe`, `GET /ws/{jobId}`, `GET /result/{jobId}`, `POST /cancel/{jobId}`. Wire callbacks → broadcast → WebSocket. | 1, 3 |
| **5** | Local storage: `~/.auohp/jobs/`, atomic `result.json` writes, job history in popover. | 4 |
| **6** | Tray icon state machine: idle → busy → complete/error. | 4 |
| **7** | Model download flow: check `~/.auohp/models/`, download via `hf-hub`, emit progress. | 4 |
| **8** | Webapp integration: GPU badge, upload UI, WS progress display, result handoff to cloud API. | 4 |
| **9** | macOS code signing, notarization, `.dmg`, GitHub Actions CI. | 6 |
| **10** | Auto-updater via `tauri-plugin-updater`. | 9 |

## 14. Open Questions

1. **File size limits:** A 2-hour interview video could be several GB. `axum::extract::Multipart` streams by default — verify with large files. Localhost transfer is fast (no network bottleneck).

2. **Port discovery:** Try-ports-in-sequence adds up to 1.5s latency. Alternative: Bonjour/mDNS (`_auohp._tcp`). More robust but adds complexity. The simple approach is probably fine for v1.

3. **Multiple browser tabs:** If caption editor and search component are both open, both discover the local service and show the GPU badge. Only one would initiate transcription. Not a problem.

4. **Embedding migration:** Switching from BGE-small (384-dim) to nomic-embed-text (768-dim) requires dropping and recreating the Neo4j `statement_embedding` vector index, then re-embedding all statements. The `IF NOT EXISTS` guard in `main.rs` will silently keep the old 384-dim index if it already exists — a `DROP INDEX` must run first during migration.
