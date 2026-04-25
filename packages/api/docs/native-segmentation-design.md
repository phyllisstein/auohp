# Design: Native ort-based Speaker Segmentation

**Status:** REQUIRED. The current code is producing unusable transcripts.
**Priority:** Blocking --- on the critical path for any real interview, not
a future cleanup.
**Author:** Daniel + Claude
**Date:** 2026-04-25

---

## 1. The bug, in one paragraph

`pyannote_rs::get_segments` returns an iterator built on `std::iter::from_fn`
that returns `None` *as soon as a single 10 s window produces zero
speech-to-silence transitions*. Standard iterator consumers (`.collect()`,
`for ... in`) interpret `None` as end-of-stream and stop. So the moment we
hit a window of continuous speech, every subsequent window is silently
discarded. On a 5 min oral history clip this drops well over half the
audio. We tried slicing the input into 30 s chunks
(`diarize.rs::chunked_get_segments`) to narrow the blast radius, but the
same bug recurses *inside each chunk* --- a 30 s chunk that opens with
continuous speech still emits zero segments. The workaround is dead weight
at this point.

## 2. What this means for the product

Concrete numbers from `/scratch/audio/026_iris_long_short.wav` (5 min
clip): we get ~22 diarization segments instead of the 200--400 you'd expect
from healthy pyannote on equivalent input.

Downstream consequences cascade:

- Speaker clustering operates on those 22 embeddings. It cannot produce
  meaningful clusters from that few samples covering that little of the
  audio. One cluster swallows everything.
- The Whisper/diarization merge in
  `pipeline.rs::merge_whisper_with_diarization` is correctly reporting
  that we have *no speaker information* for ~95% of the audio. Every
  Whisper segment falls through to `nearest_speaker_fallback`, which picks
  whichever of the two-or-three collapsed clusters has any boundary nearby
  --- in practice always the dominant one. Result: the whole transcript
  labels as one speaker.
- Downstream consumers (the editor, the search index, anything Q&A-shaped)
  see a single-speaker monologue when the audio is a clean two-speaker
  interview.

This is not a quality regression. It is a hard ceiling. The transcript
shipped through this pipeline is *wrong about who said what for almost the
entire interview*. There is no salvaging it with downstream cleverness ---
the merge code, the fallback, the clustering are all doing the right thing
with the input they receive.

## 3. Proposal

Replace the `pyannote-rs` segmentation surface with our own thin module
that calls `ort` (the ONNX Runtime crate, already a transitive dependency)
directly. Keep `pyannote-rs::EmbeddingExtractor` for now; only the
`get_segments` call has the iterator bug.

New module: `src/transcription/segmentation.rs`. New struct: `Segmenter`,
owning the `ort::Session` and the model's frame parameters.

## 4. What we keep from pyannote-rs

The model invocation pattern is correct; only the iterator bookkeeping is
wrong. Lift these pieces verbatim:

- 10 s sliding window over `i16` samples at 16 kHz
- Trailing zero-padding so the final window is full-length
- ONNX input shape: `[1, 1, samples]`, dtype f32 (cast from i16)
- Output tensor lookup by name `"output"`
- Per-frame argmax over the class axis: class 0 = silence, anything else =
  speech
- Frame-stride bookkeeping: the model emits one classification every
  `frame_size` samples, with the first frame anchored at `frame_start`

## 5. What changes

### 5.1 Bug fix: end-of-stream flush

After the final window is processed, if `is_speeching` is still true, emit
a closing segment that ends at the current `offset`. pyannote-rs never
does this, so any speech run that reaches the end of the input is lost.

### 5.2 Bug fix: don't terminate when the queue is momentarily empty

pyannote-rs's `from_fn` closure processes one window per call and returns
`pop_front()` immediately. If a window produces zero transitions the
iterator returns `None` and consumers (`.collect()`, `for ... in`) treat
that as end-of-stream, dropping every subsequent window.

The fix is to wrap the per-window processing in a `while
segments_queue.is_empty()` loop, so we keep advancing through windows
until either the queue has something to yield or the window iterator is
exhausted.

Concretely, instead of `from_fn`, expose a single
`Segmenter::segment(&mut self, samples: &[i16]) -> Result<Vec<Segment>>`
that runs to completion and returns the full vector. The caller already
collects everything into memory; we don't need streaming semantics, and
the eager API is much harder to misuse.

### 5.3 Magic numbers become named constants

```rust
/// Number of samples between consecutive output frames for
/// pyannote-segmentation-3.0 at 16 kHz input.
const FRAME_STRIDE_SAMPLES: usize = 270;

/// Sample offset of the first output frame within a 10 s window
/// (corresponds to the model's receptive-field padding).
const FIRST_FRAME_OFFSET_SAMPLES: usize = 721;

/// Window length the model expects, in seconds.
const WINDOW_SECS: usize = 10;
```

These are tuned to the specific ONNX export at
`thewh1teagle/pyannote-rs/releases/v0.1.0`. If we ever swap segmentation
models we re-derive them from the new model's published frame rate; today
they're undocumented inside `pyannote-rs/src/segment.rs:36-37`.

### 5.4 Session lifecycle

`Segmenter::new(model_path: &Path)` builds a single `ort::Session` and
stores it. Subsequent `segment()` calls reuse it. pyannote-rs already
does this internally, but because we're forced to reinstantiate
`pyannote-rs` *per chunk* in the current workaround, we pay session-build
cost (model parsing + graph optimization) on every chunk. The native
implementation pays it once.

## 6. What we don't change yet

- `EmbeddingExtractor` and the wespeaker ONNX model stay as `pyannote-rs`
  imports. The embedding path doesn't have the same iterator bug --- it's
  a simple `compute(&samples)` call per segment --- and the wespeaker
  FBank feature extractor (`kaldi-native-fbank` integration) is
  non-trivial to replicate. Defer.
- The clustering and merge code in `diarize.rs` (Phases 2--3) is correct
  and reused unchanged.
- `pyannote-rs` stays in `Cargo.toml` for the embedding path. Once we
  replicate that too we can drop the dep entirely.

## 7. Migration

1. Add `src/transcription/segmentation.rs` with `Segmenter` and the
   bug-fixed loop.
2. In `diarize.rs`:
   - Replace the `chunked_get_segments` call with `Segmenter::segment`.
   - Convert the returned `Segment`s into the `(start, end, samples)`
     tuples the existing code expects --- or change the local type to
     match `pyannote_rs::Segment`'s shape directly. The downstream
     `EmbeddingExtractor::compute(&seg.samples)` call needs the i16
     samples for that segment, so we keep the same fields.
3. Delete `chunked_get_segments` and the comment block above it.
4. Verify on `/scratch/audio/026_iris_long_short.wav`: expect
   substantially more than 22 segments, with no chunk-boundary gaps.
5. Re-run the seed pipeline on a real interview and confirm the
   transcript shows alternating speakers in Q&A passages, not a single
   dominant label.

## 8. Risk

The model output tensor layout (`[batch, time_frames, classes]`) is what
pyannote-rs assumes, and we'd be assuming the same. If the ONNX export
ever changes shape, both implementations break the same way. Low risk,
high blast radius.

The `frame_size`/`frame_start` constants are empirically tuned to the
specific ONNX export. If we accidentally pull a different segmentation
model into `models/`, our segment timestamps will drift. Mitigate by
checksumming the model file at `Segmenter::new` and refusing to load
unexpected hashes.

## 9. Future: replace `EmbeddingExtractor` too

Once segmentation is native, the wespeaker embedding extractor is the
only remaining `pyannote-rs` surface. Replicating it requires:

- FBank feature extraction (use `kaldi-native-fbank` directly ---
  `pyannote-rs` already pulls it in)
- ONNX session for the wespeaker model
- L2-normalization of the output embedding

That's ~50 lines and unblocks dropping `pyannote-rs` entirely. Defer
until the segmentation rewrite is proven.
