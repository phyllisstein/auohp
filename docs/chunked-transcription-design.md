# Design: Chunked Transcription Pipeline

**Status:** Proposal (not yet implemented)
**Author:** Daniel + Claude
**Date:** 2026-03-07

---

## 1. Context

The current transcription pipeline (`pipeline::run`) decodes the entire audio
file into memory, passes the full sample buffer to `whisper::transcribe`, then
passes it again (as i16) to `diarize::diarize`. For a 120-minute AUOHP
interview at 16 kHz mono, that's ~440 MB of f32 samples held in memory for the
duration of both inference passes.

whisper.cpp processes audio in internal 30-second windows regardless of input
length, so feeding it the full buffer is not *wrong* — but it forecloses
parallelism, streaming, and browser-upload workflows where the full file may
never exist on disk at once.

### Future constraint: browser uploads

The next major feature is a GraphQL mutation (or REST endpoint) that accepts
audio uploaded from a browser. This changes the input from "a path on the local
filesystem" to "a stream of bytes arriving over HTTP, possibly over minutes."
The chunked design should not *require* the full file to be buffered to disk
before transcription begins.

---

## 2. Proposed Architecture

```
Browser ─── HTTP multipart ──┐
                              ▼
Local file ── decode ──► SampleBuffer (ring or growable)
                              │
                         silence-detect & split
                              │
                    ┌─────────┼─────────┐
                    ▼         ▼         ▼
               [chunk 0] [chunk 1] ... [chunk N]   (each ~5 min + overlap)
                    │         │         │
                    ▼         ▼         ▼
                Whisper   Whisper   Whisper         (sequential on CPU,
                    │         │         │            parallel on GPU)
                    ▼         ▼         ▼
                 stitch timestamps & deduplicate overlap
                              │
                              ▼
                    WhisperSegment[] (full timeline)
                              │
              ┌───────────────┤
              ▼               ▼
         diarize()     (unchanged — already
              │          processes internally
              ▼          in sliding windows)
     DiarizedSegment[]
              │
              ▼
    merge_whisper_with_diarization()
              │
              ▼
    TranscriptionResult
```

### 2.1 Chunking strategy

Split the decoded 16 kHz mono f32 buffer into chunks of approximately
**5 minutes** (4,800,000 samples), with **15 seconds of overlap**
(240,000 samples) on each side.

Why 5 minutes:
- Long enough for Whisper to build useful decoder context within a chunk.
- Short enough that individual chunks fit comfortably in GPU memory alongside
  the model weights.
- Produces ~18–24 chunks for a 90–120 minute interview — a reasonable number
  to manage.

Why 15-second overlap:
- whisper.cpp uses the last ~5 seconds of each internal 30-second window as
  carry-over context for the next. With 15 seconds we guarantee at least one
  full internal window of redundant transcription at each boundary, giving us
  enough duplicate text to align and deduplicate.

Split points should prefer silence when possible: scan the overlap region for
the lowest-energy 200ms window and cut there. Fall back to the fixed boundary
if no silence is found (e.g., continuous speech). This avoids cutting mid-word,
which creates the worst stitching artifacts.

### 2.2 Stitching

Each chunk produces `WhisperSegment[]` with timestamps relative to the chunk's
start. Before stitching:

1. **Re-base timestamps** to the global timeline by adding the chunk's offset.
2. **Trim the overlap region**: for adjacent chunks A and B with a 15-second
   overlap, find the point in the overlap where both chunks produced the same
   word sequence (longest common subsequence on the word texts). Drop
   everything after that point from A and everything before it from B.
3. **Fallback**: if no matching word sequence is found (rare, but possible with
   music or noise in the overlap), split at the midpoint of the overlap region
   and accept the discontinuity.

This is the hardest part of the design. Getting it wrong produces duplicated
sentences, dropped words, or timestamp jumps. The word-level DTW timestamps
from Whisper are the key signal — they let us align overlap regions at word
granularity rather than segment granularity.

### 2.3 Diarization

No change. `pyannote_rs::get_segments` already processes audio in internal
sliding windows (~10 seconds each). It handles 120-minute buffers fine. The
speaker embedding extraction in `diarize::diarize` is already per-segment.
Chunking the diarization pass would add complexity for no measurable gain.

### 2.4 Concurrency model

```
                          ┌─── Whisper chunk 0 ───┐
                          │    Whisper chunk 1 ────│── GPU: parallel streams
                          │    ...                 │   CPU: sequential
  decode ─► split ───────►├─── Whisper chunk N ───┘
                          │
                          └─── diarize (full) ─────── always concurrent with
                                                      Whisper, shares no state
```

- **CPU (no feature flag):** Whisper chunks run sequentially. Diarization runs
  on a second `spawn_blocking` thread concurrently with the Whisper pass.
  Wall-clock improvement: ~15–25% (diarization overlaps with Whisper).

- **GPU (`--features metal` or `--features cuda`):** Whisper chunks can be
  dispatched to separate inference states on the same model context.
  whisper.cpp's `WhisperContext` is shareable; each `WhisperState` is
  independent. In practice, GPU memory limits this to 2–3 concurrent chunks
  on consumer hardware. Diarization still runs on CPU concurrently.

Gate total concurrency with a `tokio::sync::Semaphore` (1 permit for CPU,
2–3 for GPU) to prevent OOM from concurrent transcription jobs.

### 2.5 Browser upload path (future)

The chunked design enables — but does not require — streaming transcription
from browser uploads:

1. The upload endpoint accepts `multipart/form-data` or a chunked
   `application/octet-stream`.
2. Bytes are fed into symphonia's decoder incrementally (symphonia's
   `MediaSourceStream` accepts any `Read` impl, including a bounded channel
   or ring buffer).
3. As decoded samples accumulate, silence detection runs on a rolling window.
   When a chunk boundary is identified, the chunk is dispatched to the Whisper
   queue immediately — transcription begins before the upload finishes.
4. The upload endpoint returns a job ID. The client polls or subscribes (via
   GraphQL subscription over WebSocket) for `ProgressEvent` updates.

This is a significant amount of additional machinery (incremental decoding,
backpressure between upload and processing, job management). The chunked
Whisper pipeline is a prerequisite for it but does not commit us to building
it.

---

## 3. Changes to Existing Code

### New files

| File | Purpose |
|---|---|
| `transcription/chunk.rs` | Silence-aware splitting + overlap stitching |

### Modified files

| File | Change |
|---|---|
| `transcription/pipeline.rs` | `run()` calls chunk → fan-out Whisper → stitch → diarize → merge. New `run_chunked()` or refactor of existing `run()`. |
| `transcription/whisper.rs` | `transcribe()` unchanged. New `transcribe_chunk()` that accepts an offset parameter for timestamp rebasing, or just rebase after the fact in the stitcher. |
| `transcription/types.rs` | Add `TranscriptionPhase::ChunkingN` or per-chunk progress tracking. |
| `transcription/mod.rs` | Export new module. |

### Unchanged files

| File | Why |
|---|---|
| `transcription/audio.rs` | Decoding is already a separate step. No change needed. |
| `transcription/diarize.rs` | Receives the full sample buffer as today. |
| `graphql/mutations/seed_interview.rs` | Consumes `TranscriptionResult` — shape is unchanged. |
| `bin/transcribe.rs` | Calls `pipeline::run()` — if we refactor in place, this just works. |

---

## 4. Honest Assessment

### What you gain

| Gain | Magnitude | Confidence |
|---|---|---|
| Whisper + diarize run concurrently | 15–25% wall-clock on CPU | High — these are independent, and diarization is ~15% of total time per the phase weights in `types.rs`. |
| GPU parallelism across chunks | 30–50% wall-clock with 2–3 concurrent GPU streams | Medium — depends on GPU memory and whisper.cpp's actual multi-state performance, which is undertested. |
| Unblocks streaming from browser uploads | Prerequisite satisfied | High — but the streaming upload machinery is a separate, larger project. |
| Better boundary quality via silence-aware splitting | Marginal | Low — whisper.cpp's internal chunking already handles boundaries reasonably. Manual chunking might *introduce* new stitching bugs that whisper.cpp's internal logic avoids. |

### What you lose

| Cost | Magnitude | Confidence |
|---|---|---|
| Stitching correctness | **High risk** | High — overlap deduplication is the single hardest problem in this design. Off-by-one errors in word alignment produce duplicated or dropped sentences. This is a well-known problem in production ASR systems (YouTube, Otter.ai, etc.) and none of them got it right on the first try. |
| Conceptual simplicity | Significant | The current pipeline is ~125 lines of straight-line code that anyone can read top to bottom. Chunking adds splitting, rebasing, LCS-based deduplication, and concurrency coordination. The pipeline module likely triples in size. |
| Debugging difficulty | Significant | When a transcript has a glitch at minute 47, you now have to determine: was it a Whisper hallucination, a chunk boundary artifact, a stitching bug, or a diarization misalignment? Today there's only one suspect (Whisper). |
| Test surface | Moderate | You need test fixtures for: silence detection, overlap stitching, edge cases (speech across boundaries, music, applause), and the concurrency fan-out. The current pipeline has no tests (the `bin/transcribe.rs` is a manual acceptance tool). |
| Maintenance burden on pyannote-rs/ort pin | None | Chunking doesn't change the dependency situation. |

### The case against doing it now

The current pipeline processes a 90-minute interview in one `spawn_blocking`
call. It works. The output quality is acceptable. The code is simple.

The **only** hard reason to chunk is the browser-upload streaming path, which
we're explicitly not building yet. Every other benefit (concurrency, GPU
parallelism) is an optimization on a pipeline that runs infrequently — these
are oral history interviews, not a real-time transcription service. Shaving 30%
off a 20-minute job saves 6 minutes per interview. There are ~200 interviews
in the AUOHP archive.

The stitching logic is where this design could go badly wrong. It's the kind
of code that works perfectly on test fixtures and then produces subtle,
hard-to-detect errors on real interviews — a repeated sentence here, a
swallowed clause there. These errors are *worse* than Whisper's native
boundary artifacts because they happen at unpredictable points determined by
silence detection, and they corrupt the word-level timestamps that the caption
editor depends on.

### The case for doing it now

If you *are* going to build browser uploads eventually, retrofitting chunking
onto a monolithic pipeline is harder than designing it in from the start. The
`run()` function's signature (`input_path: &Path`) bakes in the assumption
that the full file exists on disk. Changing that later means changing the
function signature, the progress model, and the concurrency model all at once
— a riskier refactor than building it incrementally now.

The intermediate step — chunking from a fully-decoded in-memory buffer, no
streaming yet — is a reasonable scope that delivers the concurrency win and
validates the stitching logic without tackling incremental decoding.

---

## 5. Recommendation

**Do the intermediate step**: implement chunking and stitching over the
fully-decoded sample buffer, without streaming upload support. This is roughly
scoped as:

1. `chunk.rs`: silence-aware split (~80 lines)
2. `chunk.rs`: overlap stitching with word-level LCS (~120 lines)
3. `pipeline.rs`: refactor `run()` to decode → chunk → fan-out Whisper →
   stitch → diarize → merge (~60 lines changed)
4. Test fixtures: at least 3 cases (clean split on silence, split mid-speech,
   no-match fallback) (~100 lines)

Estimated new/changed code: ~400 lines, concentrated in one new file and one
modified file.

**Do not** build the streaming upload path, the incremental decoder, or the
job queue yet. Those are separate designs that depend on this one but multiply
the scope by 3–4x.

**Do not** parallelize Whisper chunks on GPU yet. Get stitching right first
with sequential chunk processing. GPU fan-out is a one-line change once the
sequential path is solid (`std::thread::scope` with multiple threads vs. a
single-threaded loop).

---

## 6. Open Questions

1. **Overlap width**: 15 seconds is a guess. Should we test with 10s and 20s
   to find the minimum that reliably produces matching word sequences?

2. **LCS vs. fuzzy matching**: Exact word-sequence matching for overlap
   deduplication is brittle if Whisper produces slightly different text for the
   same audio in different contexts (it does, especially for proper nouns and
   filler words). Should we use edit-distance-based alignment instead?

3. **Chunk size for GPU**: 5 minutes is tuned for CPU sequential processing.
   On GPU, shorter chunks (2–3 min) with more parallelism might be faster.
   Should chunk size be configurable via `PipelineConfig`?

4. **Progress model**: Currently `TranscriptionPhase::Transcribing` reports a
   single 0.0–1.0 progress. With chunks, do we report per-chunk progress
   (more granular) or aggregate (simpler)?
