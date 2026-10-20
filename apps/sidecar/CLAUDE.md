# `@auohp/desktop` --- local capability broker

## What this package is

A tray-resident daemon that lends the user's own hardware to the AUOHP editorial
webapp. It is not a desktop version of the editor, and it is not a general-purpose
app. It brokers two things the browser cannot reach: the local filesystem and the
local GPU.

The design hypothesis is economic. ACT UP's corpus grows at the rate volunteers can
edit it, so batch throughput was never the constraint. A three-year-old MacBook Pro
runs inference competitively; what it cannot do is train, or scale horizontally.
Running inference on hardware the project already owns means no cloud GPU bill and no
spot instances to babysit --- which matters more than performance for a single-developer
project.

## This architecture is settled

Open to refinement, closed to relitigation. It was arrived at deliberately; treat it as
the premise of the work, not as a proposal awaiting review. Do not reopen the transport
split, the consent boundary, or the actor boundaries at the start of each session, and do
not offer alternatives unasked. If an instruction here turns out to be genuinely
impossible or contradicted by the code, say so once, plainly, and proceed.

## The actors

Three, with hard boundaries between them:

- **The editorial webapp** (`packages/editor`, served from `editor.actupny.com`) ---
  the only client of this package's API. Owns persistence, remote services, and
  everything downstream of a finished transcription.
- **The local HTTP API** (axum, loopback) --- the webapp's door into this machine.
- **The tray UI** --- a small webview the user owns. Progress, cancel, model and file
  selection, auth. Deliberately a separate artifact from the editor, on its own build,
  free to drift.

## The flow

1. Webapp asks the local API to create a job. It supplies an interview identifier and
   job config. **It never supplies a file path** --- a remote page must not know
   absolute paths on the user's disk.
2. The tray icon signals that attention is needed. Nothing else happens yet.
3. The user clicks the tray and picks a file. This native gesture is the consent
   boundary (see below).
4. Inference runs locally. Progress streams to the tray UI and, separately, back to
   the webapp.
5. The result is handed to the webapp, which persists it.

## The consent boundary

The tray gesture in step 3 is the architectural centrepiece, not a UX detail. A remote
page is causing native effects --- a dialog opening, a GPU spinning for an hour. Nothing
native happens without a gesture on the native surface first.

This is the browser's user-activation rule rebuilt one layer down, because the browser's
version stops at the tab.

**Legibility is a security property here.** Sanding down the seam between webapp and
desktop app makes the trust relationship *less* clear, not more. The user should be able
to see both halves and know which one is holding the knife --- "I could close the tab and
the remote server still isn't reaching into my laptop." Prefer visible demarcation over
smoothness whenever they conflict.

Corollaries: the local API is not a general RPC surface. Origin checking and pairing are
not deferred hardening --- they are the feature. A pairing token is naturally minted at
the consent gesture.

## Reading the current tree

Much of `src/` predates this architecture and mistakes infrastructure for architecture.
Do not infer intent from it.

Sound and load-bearing:

- The single-slot job registry, its cleanup wrapper, and the cooperative cancellation
  token. The concurrency model is correct.
- `TranscribeSource` as the convergence type for job inputs.
- The SSE events route. This is the progress channel to the webapp --- the most important
  route in the file, and the least developed.
- `GET /health` as capability negotiation. Correct shape, fictional contents (hardcoded
  version, capabilities, and model names). Capability discovery rather than version
  lockstep is what lets the two artifacts drift.

Known wrong, do not preserve:

- The `#[tauri::command]` surface mirrors the HTTP API (submit / cancel / status). Those
  are the webapp's concerns and the webapp will never speak IPC. The commands the tray UI
  needs are a different set --- file picking, model enumeration, auth, progress
  subscription --- and none of them exist. Delete rather than repair.
- The job-creation request type accepts a filesystem path from the webapp. See step 1.
- The tray's "Say Hello" handler makes a loopback HTTP round-trip from Rust to Rust in
  the same process. It is a symptom of the missing architecture, not a pattern.
- Five Tauri plugins are compiled in and never installed. `single-instance` in particular
  guards a real failure mode for a port-binding daemon.
- No exit handling. Quit kills detached tasks mid-decode --- no terminal event, no partial
  result, bypassing the cooperative cancellation that `worker.rs` carefully implements.
- Cancellation is coarse. The blocking decode thread runs to completion after its handle
  is abandoned; true mid-decode cancellation needs the token plumbed into whisper's abort
  callback inside `auohp-core`.

## Non-goals

- Rendering or editing transcripts. That is the editor's job.
- Talking to Neo4j. The desktop app hands results to the webapp and forgets them.
- Multi-job concurrency. One job at a time is a deliberate constraint on a machine the
  user is also using for other things.
- Sharing a build, a dependency tree, or a version with `packages/editor`.
- Durable local state. No on-disk checkpoint of in-flight or completed transcripts, and
  no resume-after-crash. A file implies a schema, a flush policy, a migration story, and
  a reconciliation story for realigning stale local data against a transcript the user
  has since edited upstream --- decisions that would exert real gravity on every later
  design, in exchange for saving a re-run. The desktop app's memory is the process
  lifetime. If it dies, the job is re-run.

## Open questions

Known, not yet built for. Be aware of these; don't pre-emptively design around them.

- **Result delivery is not durable.** A finished transcription exists exactly once,
  inside the terminal event, sent into a broadcast channel that keeps no history. A
  subscriber that is absent at that instant --- a webapp reconnecting its SSE stream, a
  closed tab --- misses it, and the send error is discarded. An hour of GPU can evaporate
  silently.

  This is a *retention* problem, not a *persistence* one, and the distinction is what
  keeps it out of the non-goal above: the fix is for the registry to hold a completed
  job until someone collects it, in memory, for the life of the process. No format, no
  flush policy, no realignment. It implies the slot models more than "occupied or
  empty", which is a real change to the concurrency model and should be made
  deliberately rather than in passing.
