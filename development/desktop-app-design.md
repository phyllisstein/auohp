# AUOHP Desktop App — Design Document

**Status:** Draft — not yet started
**Last updated:** 2026-03-20

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

A hidden-by-default Tauri window (~320×400px) that shows:
- Current job stage and progress bar
- Live transcript segments as they arrive
- Model download progress (first run)
- Error details

This is a **display-only** surface — no interactive controls, no preferences, no forms. The user reads it; they don't operate it. This matters because the popover is a Tauri webview (HTML/CSS rendered by WebKit on macOS), not native AppKit. Interactive controls (text fields, dropdowns, segmented controls) would feel wrong if non-native; a passive status display sidesteps those expectations entirely.

The tray menu itself (section 4.2) is native — Tauri's `tauri::tray::TrayIcon` and `tauri::menu::Menu` delegate to `NSMenu` on macOS, so it's indistinguishable from any other tray app.

To approximate a native macOS popover appearance:

```rust
let popover = tauri::WebviewWindowBuilder::new(app, "status", url)
    .title("")
    .inner_size(320.0, 400.0)
    .transparent(true)           // allows vibrancy to show through
    .decorations(false)          // no title bar — looks like a popover, not a window
    .build()?;
```

With `transparent: true`, `decorations: false`, and a CSS background of `transparent` over Tauri's `NSVisualEffectView` integration, the popover gets the frosted-glass translucency of a native macOS panel. Combined with the `-apple-system` font stack and `@media (prefers-color-scheme: dark)`, it reads as "system UI" rather than "web page."

## 5. HTTP API (embedded axum)

### `GET /health`

```json
{
  "status": "ok",
  "version": "0.2.0",
  "capabilities": ["transcribe", "diarize", "embed"],
  "backend": "metal",
  "gpu": { "name": "Apple M2 Pro", "memory_bytes": 17179869184 },
  "models": {
    "whisper": "large-v3-turbo-q8",
    "diarization": "pyannote-segmentation-3.0",
    "embeddings": "nomic-embed-text-v1.5"
  }
}
```

The webapp calls this on page load to detect local inference availability. The response uses a **capabilities-based handshake** rather than version comparison — the webapp asks "can you do X?" not "are you version Y?". This decouples the webapp's feature expectations from the desktop app's release cadence: the webapp can gracefully degrade when a capability is absent ("Local GPU available for transcription. Knowledge graph tagging requires app update.") without needing to maintain a version compatibility matrix.

- **`backend`** is determined at compile time via `cfg!(feature = "metal")` — the feature flag is the declaration of which backends were linked. See section 9.1.
- **`gpu`** is determined at runtime via `metal::Device::system_default()` on macOS (chip name, unified memory budget). Useful for debugging and for the webapp to warn about large files on constrained hardware.
- **`capabilities`** is derived from which models are actually present on disk — the binary may support embedding but the user hasn't downloaded the nomic model yet.
- **`models`** reports which model variants are loaded, for observability.

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

### Platform-correct paths

All local storage uses Tauri's `AppHandle::path()` API, which delegates to each platform's standard directory conventions. No dotfile directories in `$HOME`.

```rust
let data_dir = app.path().app_data_dir()?;   // user data that can't be regenerated
let cache_dir = app.path().app_cache_dir()?;  // re-downloadable files (models)
let log_dir = app.path().app_log_dir()?;      // logs
```

| Purpose | macOS | Linux | Windows |
|---|---|---|---|
| Job results | `~/Library/Application Support/com.auohp.desktop/` | `~/.local/share/com.auohp.desktop/` | `%APPDATA%\com.auohp.desktop\` |
| Models (cache) | `~/Library/Caches/com.auohp.desktop/` | `~/.cache/com.auohp.desktop/` | `%LOCALAPPDATA%\com.auohp.desktop\` |
| Logs | `~/Library/Logs/com.auohp.desktop/` | `~/.local/share/com.auohp.desktop/logs/` | `%APPDATA%\com.auohp.desktop\logs\` |

The subdirectory name (`com.auohp.desktop`) comes from the bundle identifier in `tauri.conf.json`.

**Models go in the cache directory.** They're re-downloadable. macOS may purge `~/Library/Caches/` under storage pressure, which is desirable — the app re-downloads on next use. Job results go in the data directory because the original audio may have been deleted.

### Job storage layout

```
<app_data_dir>/jobs/
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

### Live transcript streaming (mid-term goal)

The segment callback provides a natural source for live text in the status popover: as Whisper finishes each segment, its text replaces the previous line below the progress bar. This gives the user immediate confidence that transcription is working and producing reasonable output — if they glance at the popover for a second or two, they see real words flash by. The `{ "type": "segment" }` WebSocket events (section 5) already carry the text; the popover just needs to display the most recent one.

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

### GPU backend detection and the build matrix

The hardware acceleration backend is a **compile-time** decision, not a runtime query. The `metal` and `cuda` feature flags in `auohp-api/Cargo.toml` forward to sub-features in dependencies:

```toml
[features]
metal = ["whisper-rs/metal", "ort/coreml"]
cuda = ["whisper-rs/cuda", "ort/cuda"]
```

This is feature forwarding — the flags don't gate any `#[cfg]` code in the pipeline itself. Their only job is to reach into the dependency tree and flip compile-time switches: `whisper-rs/metal` builds whisper.cpp with `-DWHISPER_METAL=ON` and links `Metal.framework`; `ort/coreml` downloads an ORT binary with the CoreML EP. The pipeline code calls the same `WhisperContext::new_with_params()` regardless — the feature flag is invisible at the API level.

**This means separate binaries per backend.** When built with `--features cuda`, the binary dynamically links against `libcublas` and `libcudart`. If those aren't on the user's machine, the process fails at load time — the OS linker resolves symbols eagerly, before `main()` runs. There's no graceful fallback. A CUDA binary can't run on a non-CUDA machine.

The practical CI build matrix:

| Target | Feature flags | Artifact |
|---|---|---|
| `aarch64-apple-darwin` | `metal` | `.dmg` (Apple Silicon) |
| `x86_64-pc-windows-msvc` | `cuda` | `.msi` (Windows + NVIDIA) |
| `x86_64-pc-windows-msvc` | (none) | `.msi` (Windows CPU-only) |

For v1 (macOS-only), this is a single build. `tauri-plugin-updater` serves the correct artifact per target triple from the update manifest.

**The `backend` field in `/health`** is determined at compile time:

```rust
fn gpu_backend() -> &'static str {
    if cfg!(feature = "metal") { "metal" }
    else if cfg!(feature = "cuda") { "cuda" }
    else { "cpu" }
}
```

`cfg!` evaluates at compile time and collapses to a `bool` literal — dead branches are eliminated entirely. The binary doesn't contain the string `"cuda"` when built with `metal`.

**The `gpu` field in `/health`** is runtime hardware introspection. On macOS with Metal:

```rust
fn metal_gpu_info() -> Option<(String, u64)> {
    let device = metal::Device::system_default()?;
    let name = device.name().to_string();
    let vram = device.recommended_max_working_set_size();
    Some((name, vram))
}
```

`Device::system_default()` returns `Option<Device>` — `None` on machines without Metal support, giving detection and degradation in one call.

### Model files and sizes

| Model | Size | Download mechanism |
|---|---|---|
| `ggml-large-v3-turbo-q8_0.bin` (Whisper) | ~874 MB | `hf-hub` crate, first run |
| `pyannote-segmentation-3.0.onnx` | ~17 MB | Bundle in `.dmg` or download |
| `wespeaker_en_voxceleb_CAM++.onnx` | ~80 MB | Bundle in `.dmg` or download |
| nomic-embed-text-v1.5 (fastembed) | ~270 MB | Automatic via `fastembed` |

**Model files are backend-agnostic.** The same `ggml-large-v3-turbo-q8_0.bin` runs on CPU, Metal, and CUDA — the format describes tensor shapes and quantization; the backend decides how to execute. The same `.onnx` files run on any ORT execution provider. The model cache directory is identical across all platforms and backends.

### First-run flow

1. App launches → checks the cache directory (see section 6) for expected model files.
2. Missing models trigger the download screen (status popover opens automatically).
3. Download progress events are emitted on any active WebSocket and displayed in the popover.
4. Pyannote models (~97 MB combined) could be bundled in the `.dmg` to reduce first-run friction. Whisper model is too large to bundle.

### Storage management

Status popover shows total model storage and offers "Delete Models" to clear the model cache directory and the fastembed HF cache.

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

### Evergreen auto-update

The app uses `tauri-plugin-updater` to stay current silently, approximating Chrome's "always up to date" UX. The user never sees an update prompt or makes an update decision.

**Why full app replacement is the only viable approach on macOS:** A signed and notarized `.app` bundle has its integrity verified by Gatekeeper via a hash of every executable, dylib, and resource inside it. Replacing or modifying any file within the bundle invalidates the code seal — macOS will refuse to launch the app ("damaged app" dialog). Hot-swapping internal binaries, sidecar executables, or bundled dylibs is not possible without re-signing and re-notarizing the entire bundle. `tauri-plugin-updater` handles this correctly: it downloads a complete, pre-signed, pre-notarized `.tar.gz` and replaces the `.app` in `/Applications/`.

**Update flow:**

```
App launches
  → Spawn background update check (no UI, no prompt)
  → If update available:
      → Download silently to staging area
      → When download completes:
          → If idle (no active transcription):
              → app_handle.restart()  // immediate silent swap
          → If transcribing:
              → Set pending_restart flag
              → When job completes → app_handle.restart()
```

`AppHandle::restart()` exits the process and relaunches. For a tray app with no visible window, the user sees the tray icon briefly disappear and reappear — that's the entire visible UX.

On the first launch of a new bundle, macOS Gatekeeper re-verifies the code signature against the notarization ticket (~1–3 seconds on Apple Silicon). This is the only perceptible artifact of an update: a slightly slower tray icon appearance on the first post-update launch. For a login-item tray app, this happens during boot when nobody is watching.

**Update source:** The app checks a JSON manifest hosted on GitHub Releases independently on each launch. The webapp is not involved in triggering updates — the app is self-sufficient in staying current. This covers the "never opens the website" user who leaves the tray app running for weeks.

**Model-binary version coupling:** A new app version may require different model formats (e.g., a whisper-rs upgrade that changes the expected GGML format). On first launch after update, the app checks a bundled `models-manifest.json` that maps required model files to expected checksums, and re-downloads any stale models from the cache directory.

**Tray state machine addition:** The state machine (section 4.2) includes an implicit "update pending" state: if an update downloads while a job is running, the restart is deferred until job completion. No additional tray UI is needed — the user doesn't know or care.

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
| Storage | Platform data directory (section 6) | S3/R2 bucket with TTL |
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
| **5** | Local storage: platform data directory (section 6), atomic `result.json` writes, job history in popover. | 4 |
| **6** | Tray icon state machine: idle → busy → complete/error. | 4 |
| **7** | Model download flow: check cache directory, download via `hf-hub`, emit progress. | 4 |
| **8** | Webapp integration: GPU badge, upload UI, WS progress display, result handoff to cloud API. | 4 |
| **9** | macOS code signing, notarization, `.dmg`, GitHub Actions CI. | 6 |
| **10** | Auto-updater via `tauri-plugin-updater`. | 9 |

## 14. Open Questions

1. **File size limits:** A 2-hour interview video could be several GB. `axum::extract::Multipart` streams by default — verify with large files. Localhost transfer is fast (no network bottleneck).

2. **Port discovery:** Try-ports-in-sequence adds up to 1.5s latency. Alternative: Bonjour/mDNS (`_auohp._tcp`). More robust but adds complexity. The simple approach is probably fine for v1.

3. **Multiple browser tabs:** If caption editor and search component are both open, both discover the local service and show the GPU badge. Only one would initiate transcription. Not a problem.

4. **Embedding migration:** Switching from BGE-small (384-dim) to nomic-embed-text (768-dim) requires dropping and recreating the Neo4j `statement_embedding` vector index, then re-embedding all statements. The `IF NOT EXISTS` guard in `main.rs` will silently keep the old 384-dim index if it already exists — a `DROP INDEX` must run first during migration.

5. **Version skew between webapp and local app.** The capabilities-based `/health` handshake (section 5) handles feature presence, but doesn't cover protocol changes. If the WebSocket event schema evolves (new event types, renamed fields), an old desktop app may misparse events from a newer webapp. Consider versioning the API path (`/v1/transcribe`, `/v2/transcribe`) or including a `protocol_version` field in `/health` for the webapp to negotiate against.

6. **The "never updates" user.** Non-technical users in nonprofit orgs may dismiss update prompts indefinitely — or more likely, the tray app may be force-quit and never relaunched, so the silent update never runs. If the webapp evolves past the minimum supported protocol version, it should stop offering local transcription and show "Please reopen your AUOHP app" rather than silently breaking.

7. **macOS permission re-prompts.** Replacing the `.app` bundle can trigger macOS to re-prompt for permissions (network access, file access) because macOS ties permissions to the code signature hash, which changes with each build. Using a consistent `designated requirement` in code signing mitigates this but doesn't eliminate it across major version bumps.

8. **Update rollback.** `tauri-plugin-updater` has no built-in rollback. If a new version has a regression (Metal crash on older M1, model format incompatibility), the only recovery path is shipping a hotfix. Consider keeping the previous `.app` as `.app.bak` and offering a "Revert to previous version" option — or accept that fast CI turnaround is the mitigation.
