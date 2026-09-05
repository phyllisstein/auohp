# Desktop sidecar: first end-to-end slice through all three legs

**Status:** Design — not yet started
**Last updated:** 2026-09-03
**Relationship to earlier docs:** Implements the architecture in `packages/desktop/CLAUDE.md`.
Supersedes the transport/upload details in `2026-03-10 - desktop-app-design.md` (that doc
predates the settled consent-boundary architecture: WebSocket, multipart upload, and
webapp-supplied paths are all out).

## Context

`packages/desktop` is the local capability broker described in `packages/desktop/CLAUDE.md`:
a tray-resident daemon that lends the user's Mac to the AUOHP editorial webapp for
inference. The architecture is settled. What exists today is infrastructure, not the
architecture: a sound single-slot job registry (`src/transcription/`), a `#[tauri::command]`
surface that wrongly mirrors the HTTP API, an axum server on `127.0.0.1:8705`, and a tray
icon that opens a dropdown `Menu`. There is **no tray webview**, and the two-phase job flow
(webapp requests → user authorizes with a native gesture → inference runs) is not built.

This slice wires one datum — a single audio file's transcription — through all three legs
end to end, proving the plumbing without the real ML:

- **Leg A** webapp → Rust over HTTP: `POST /transcribe/request` with `{ interviewId, config }`, **no path**.
- **Leg B** tray webview ↔ Rust over Tauri IPC: the user picks a file (the consent gesture), progress streams back.
- **Leg C** Rust → webapp over SSE: progress events, then a final `Done` payload the webapp could persist via `seedInterview`.

The transcription itself is **stubbed** for this slice (timed `Stage` events + a hardcoded
2-segment result). The file is really picked; it is just not really decoded. Swapping the
real `auohp-core` pipeline back in is a later slice and touches exactly one function.

### Decisions locked during design

- **Pending request is single-slot, replace-on-new.** `Mutex<Option<PendingRequest>>`; a fresh `POST` overwrites an un-authorized pending request. No queue.
- **Browser transport is SSE**, not WebSocket. Keeps the HTTP surface legible (`POST` to act, `GET` to observe); native auto-reconnect.
- **Result retention is out of scope.** If the browser disconnects it misses the result. The registry keeps its current `Option<Running>` slot and `slot.take()` on cleanup. (Known hole, already documented in `packages/desktop/CLAUDE.md` "Open questions".)
- **Tray frontend is plain HTML + vanilla JS** via `window.__TAURI__` (`withGlobalTauri` already on). No build step, no `package.json`, no framework.
- **Job trigger is a button on `packages/editor/src/routes/transcript/create.tsx`.** Stub `interviewId` and default config.
- **The consent boundary is the `PendingRequest` → `Registry::submit` transition.** Nothing native (dialog, worker) happens before the tray gesture.

## Architecture

Two pieces of desktop-process state, one per trust level:

| State | Type | Written by | Carries |
|---|---|---|---|
| `PendingRequest` slot | `Mutex<Option<PendingRequest>>` (new) | `POST /transcribe/request` handler | request id, `interview_id`, `TranscribeConfig`, a pre-job status the webapp can poll |
| `Registry` slot | `Mutex<Option<Running>>` (**unchanged**) | `pick_file_and_start` command | job id, `TranscribeSource`, cancel token, `broadcast::Sender<Event>` |

`PendingRequest` and `Running` are deliberately **not** one enum: disjoint fields, disjoint
owners, and the consent transition becomes a move between structures rather than a field
mutation — you cannot hold a `Running` without having gone through `submit`.

Lifecycle:

```
POST /transcribe/request   → PendingSlot = Some(req)          Registry = None
tray badge lights, user opens panel
user picks file (pick_file_and_start):
    PendingSlot.take()  →  Registry::submit(Local{path, interview_id}, config)
                           PendingSlot = None                 Registry = Some(Running)
worker (stubbed) emits Stage… then Done
cleanup wrapper: terminal event sent, slot.take()             Registry = None
```

Event fan-out: the `broadcast::Receiver<Event>` from `submit` is drained twice —
`emit_to(webview_window("tray"), "job://event", …)` for leg B, and the existing
`Sse`/`broadcast_to_sse` path for leg C. One `Event` enum, one channel, two pipes.

## Desktop crate changes (`packages/desktop`)

### `Cargo.toml` / `tauri.conf.json` / `capabilities.json`

- `tauri.conf.json`: add `"build": { "frontendDist": "../desktop/tray-ui" }` (path holding the tray HTML — final location TBD by implementer, keep it inside `packages/desktop`). Keep `withGlobalTauri: true`.
- `capabilities.json`: change `"windows": ["main"]` (dead label) → `["tray"]`. Keep `core:default`. No dialog/fs JS permissions — those calls happen in Rust.
- No new crate dependencies; `tauri-plugin-dialog` is already a dependency, just unregistered.

### `src/main.rs`

- **Register the missing plugins** in `tauri::Builder`: `.plugin(tauri_plugin_dialog::init())`. (`tauri_plugin_fs::init()` not needed — frontend doesn't touch fs.)
- **Collapse the two `TrayIconBuilder::new()` calls into one.** The single builder must: forward events to `tauri_plugin_positioner::on_tray_event`; on left-click, position the panel (`move_window(Position::TrayBottomCenter)`) and toggle `.show()`/`.hide()` on the `"tray"` window. Drop the dropdown `Menu` for job actions; keep a minimal right-click menu with `Quit` only.
- **Build the tray `WebviewWindow` once in `setup()`**, label `"tray"`, `WebviewUrl::App("index.html")`, `.decorations(false).resizable(false).always_on_top(true).visible(false).focused(false).skip_taskbar(true).inner_size(360, 480).visible_on_all_workspaces(true)`.
- **Replace the command surface.** Delete `transcribe_local`, `transcribe_cancel`, `transcribe_status`. Add the tray commands below. Update `generate_handler!`.
- **`AppState`** gains the pending slot (or a second `.manage()`d type — implementer's call; a newtype `Arc<PendingSlot>` managed separately is cleaner than widening `AppState`).
- **`spawn_app_emit_bridge`**: change `app.emit("transcription://event", …)` → `app.emit_to(tauri::EventTarget::webview_window("tray"), "job://event", …)`.
- **Remove `path` from the HTTP request path.** `POST /transcribe` (job creation with a path) goes away; `POST /transcribe/request` (below) replaces it. `TranscribeRequest.path` is deleted.

### New: `PendingRequest` + pending slot

New module `src/pending.rs` (sibling to `transcription/`). Holds:
- `PendingRequest` — request id (nanoid), `interview_id`, `TranscribeConfig` (reuse `auohp_core::transcription::TranscribeConfig` — already exists, see `pipeline.rs:33`), and a `watch` or small `broadcast` channel carrying `PreJobStatus` (`waiting_for_file | superseded | picker_cancelled`).
- `PendingSlot` — `Mutex<Option<PendingRequest>>` with methods: `put(req)` (replaces, sends `superseded` on the old one's channel), `take()`, `peek()` (cheap view for `/status` and the tray command).

### `src/transcription/` changes

- **`registry.rs`, `source.rs`: unchanged** except `submit` gains a `config: TranscribeConfig` parameter it threads to `worker::run`. `TranscribeSource::Local` keeps `path` + `interview_id`.
- **`worker.rs`: stub the body.** Keep the signature and the `tokio::select! { biased; _ = cancel.cancelled() => …, … }` skeleton. Inside: three `Stage` events (`"loading model"`, `"decoding audio"`, `"aligning"`) each after a `tokio::time::sleep(Duration::from_secs(1))` that is `select!`ed against `cancel`, then return `Ok(TranscriptionResult { segments: vec![<two hardcoded Segments>] })`. Leave a `// FIXME: real pipeline` pointing at `auohp_core::transcription::run_with`. The real call (`run_with(&path, &cfg)` via `spawn_blocking`) is the only thing that changes when we un-stub.

### New tray commands (`src/main.rs` or `src/commands.rs`)

All async, all return `Result<_, _>` (required to borrow `State<'_, _>` across `.await` —
the `ResultFutureTag` path drops the `'static` bound; verified in `tauri` 2.11 macro source).

| Command | Params (types) | Returns | Behavior |
|---|---|---|---|
| `pending_request` | `State<Arc<PendingSlot>>` | `Result<Option<PendingView>, ()>` | `slot.peek()` mapped to a serializable view (request id, interview id, config summary). Drives the panel's main view. |
| `pick_file_and_start` | `State<Arc<PendingSlot>>`, `State<Arc<Registry>>`, `AppHandle` | `Result<JobId, StartError>` | **The consent gesture.** `slot.take()` → `StartError::NoPendingRequest` if empty. `spawn_blocking(move ‖ app.dialog().file().add_filter("Audio", &["wav","mp3","m4a","flac"]).blocking_pick_file())`. `None` → send `picker_cancelled`, put the request back, `StartError::Cancelled`. `Some(fp)` → `fp.into_path()`, build `TranscribeSource::Local`, `registry.submit(source, config).await` → on `Busy` put request back and return `StartError::Busy`. On success wire `spawn_app_emit_bridge(app, outcome.events)` and return the job id. |
| `cancel_job` | `State<Arc<Registry>>`, `id: JobId` | `Result<(), CancelError>` | Thin pass-through to `Registry::cancel`. |
| `job_status` | `State<Arc<Registry>>`, `State<Arc<PendingSlot>>` | `Result<TrayStatus, ()>` | Combined `idle | pending{…} | running{id, stage?}` view for the panel. |

### HTTP handlers (`src/main.rs`)

| Route | Change |
|---|---|
| `POST /transcribe/request` | **New.** Body `{ interviewId, config }`. `PendingSlot::put(PendingRequest::new(…))`. `202` → `{ requestId }`. Never touches `Registry`. |
| `GET /transcribe/status` | **Extend.** Variants: `idle`; `pending { requestId, interviewId }`; `running { id }`. Webapp polls this across the two-phase gap. |
| `GET /transcribe/events` | **Unchanged.** SSE of the running job's `broadcast` stream, `404` when idle. Late subscribers miss history (out of scope). |
| `POST /transcribe/cancel` | **Unchanged.** |
| `GET /health` | **Unchanged.** |
| `POST /transcribe`, `POST /hello` | **Delete.** |

### New: tray frontend

One file, `packages/desktop/tray-ui/index.html` (location must match `frontendDist`). Inline
`<style>` + `<script>`. Uses `window.__TAURI__.core.invoke`,
`window.__TAURI__.webviewWindow.getCurrentWebviewWindow().listen("job://event", …)`.

Behavior:
- On load and on `job://event`: call `job_status`, render one of — "Nothing waiting", "Webapp wants interview N → [Pick file & start]" button, "Running: {stage}" + [Cancel] button.
- Button → `invoke("pick_file_and_start")` / `invoke("cancel_job", { id })`.
- No routing, no framework, no build.

## Editor changes (`packages/editor`)

### New: `src/routes/transcript/-transcription-signal.ts`

`createModel` singleton `transcriptionJob` (pattern: `src/routes/search/-search-signal.ts`,
`src/playhead.ts`). Signals: `phase` (`"idle" | "pending" | "running" | "done" | "error"`),
`stage: string`, `result: TranscriptionResult | null`, `error: Error | null`. One exported
`export const transcriptionJob = new TranscriptionJob()`.

### New: `src/routes/transcript/-desktop-events.ts`

Hand-authored TS types (no codegen — drift is intentional per `packages/desktop/CLAUDE.md`).
**snake_case on the wire** (serde does not rename):
- `DesktopEvent` union tagged by `kind`: `started {id}`, `stage {id, message}`, `done {id, result}`, `cancelled {id}`, `failed {id, message}`.
- `TranscriptionResult { segments: Segment[] }`, `Segment { speaker: string | null; text: string; start_time: number; end_time: number; words: Word[] }`, `Word { word: string; start: number; end: number; p: number }` — mirrors `packages/core/src/transcription/types.rs`.

### New: `src/routes/transcript/-transcription-driver.ts` (or a render-nothing component)

Owns the imperative I/O, `batch()`-commits into `transcriptionJob` (pattern: `SearchDriver`
in `src/lexical/extensions.tsx`):
1. `startTranscription()` → `POST ${DESKTOP_URI}/transcribe/request` `{ interviewId, config }`, set `phase = "pending"`.
2. Poll `GET ${DESKTOP_URI}/transcribe/status` (~1s) until `state === "running"`.
3. Open `EventSource(${DESKTOP_URI}/transcribe/events)`. Each message → parse `DesktopEvent`, `batch()` → update `phase`/`stage`/`result`/`error`. Close the source on `done`/`failed`/`cancelled`.

`DESKTOP_URI` = `import.meta.env.VITE_AUOHP_DESKTOP_URI ?? "http://localhost:8705"`.

### `src/routes/transcript/create.tsx`

- Keep the `/health` `Badge`.
- Add a "Start transcription" `Button` (`@react-spectrum/s2/Button`) → `startTranscription()` with stub `interviewId` + default `config`.
- Read `transcriptionJob` signals: while `running`, show `ProgressCircle isIndeterminate` + the `stage` string; on `done`, show `result.segments.length` and a disabled "Save to interview" stub button (the `seedInterview` bridge is a separate slice).
- S2 import style: deep per-component (`import { Button } from "@react-spectrum/s2/Button"`).

## What is explicitly NOT in this slice

- Real transcription (stubbed worker).
- `result.segments` → `seedInterview` persistence (stub button only).
- Result retention / late-subscriber replay.
- Pairing token / origin checking at the consent gesture (named in `packages/desktop/CLAUDE.md` as "the feature" — deferred to its own slice).
- True mid-decode cancellation (worker-level `select!` only; the `auohp-core` abort callback is untouched).
- Exit handling / graceful shutdown.
- A real tray-ui build system.
- `NSPanel` semantics (the builder-flag approximation is enough).

## Verification

End to end, with the stub worker:

1. `cd packages/desktop && cargo run` (no `--features metal` needed — worker is stubbed). Menu-bar icon appears, no dock icon (`ActivationPolicy::Accessory`).
2. `cargo build` succeeds and regenerates `gen/schemas/` from the rewritten `capabilities.json`.
3. Click the tray icon → the `"tray"` panel appears anchored under the icon (positioner working proves the single-`TrayIconBuilder` fix). It shows "Nothing waiting".
4. `curl -sX POST localhost:8705/transcribe/request -H 'content-type: application/json' -d '{"interviewId":"999","config":{}}'` → `202` + `{"requestId":"…"}`.
5. `curl -s localhost:8705/transcribe/status` → `{"state":"pending","requestId":"…","interviewId":"999"}`.
6. Tray panel (re-open or auto-refreshed) now shows "Webapp wants interview 999 → [Pick file & start]".
7. Click it → native file dialog. Pick any audio file.
8. Panel switches to "Running: loading model" → "decoding audio" → "aligning" over ~3s (proves `job://event` IPC + the stub worker's `select!` skeleton).
9. In parallel, `curl -N localhost:8705/transcribe/events` (started right after step 7) streams the same `data: {"kind":"stage",…}` frames, then `data: {"kind":"done","id":"999:…","result":{"segments":[…2…]}}` (proves leg C SSE + the payload).
10. `curl -s localhost:8705/transcribe/status` → `{"state":"idle"}` (proves the cleanup wrapper vacated the slot).
11. Cancellation: repeat 4–7, and during the ~3s window click [Cancel] in the panel (or `curl -X POST …/transcribe/cancel -d '{"id":"…"}'`) → panel shows cancelled, SSE emits `{"kind":"cancelled"}`, `/status` returns to `idle` within ~1s (proves the worker `select!` honours the token).

Editor leg:

12. `cd packages/editor && yarn dev`, open `/transcript/create`. Health `Badge` shows "Healthy" (desktop running).
13. Click "Start transcription" → `POST /transcribe/request` fires (network tab). Page shows "pending".
14. Open the tray panel, pick a file. Page transitions pending → running, `stage` string updates live, then "done" with `2` segments shown (proves the editor `EventSource` + signal model — the first streaming consumer in the editor).

## Key files

**Desktop — modify:** `packages/desktop/src/main.rs`, `packages/desktop/src/transcription/worker.rs`, `packages/desktop/src/transcription/registry.rs` (signature only), `packages/desktop/tauri.conf.json`, `packages/desktop/capabilities.json`, `packages/desktop/Cargo.toml` (maybe — plugin already present).
**Desktop — create:** `packages/desktop/src/pending.rs`, `packages/desktop/src/commands.rs` (optional split), `packages/desktop/tray-ui/index.html`.
**Editor — modify:** `packages/editor/src/routes/transcript/create.tsx`.
**Editor — create:** `packages/editor/src/routes/transcript/-transcription-signal.ts`, `.../-desktop-events.ts`, `.../-transcription-driver.ts`.

**Reuse:** `auohp_core::transcription::{TranscribeConfig, TranscriptionResult, Segment, Word}` (`packages/core/src/transcription/types.rs`, `pipeline.rs`); `Registry` / `TranscribeSource` / `Event` (`packages/desktop/src/transcription/`); `tauri_plugin_positioner::{Position, WindowExt, on_tray_event}`; `createModel` (`packages/editor/src/**/createModel`); the `searchQuery` + `SearchDriver` pattern (`packages/editor/src/routes/search/-search-signal.ts`, `src/lexical/extensions.tsx`).
