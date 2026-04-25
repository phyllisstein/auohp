# Design: Native ort-based Speaker Segmentation

**Status:** Proposal (not yet implemented)
**Author:** Daniel + Claude
**Date:** 2026-04-25

---

## 1. Context

`src/transcription/diarize.rs` currently calls `pyannote_rs::get_segments` to
run the `pyannote-segmentation-3.0` ONNX model and emit speech segments. The
upstream crate has an iterator bug that silently drops every window after the
first one that produces zero speech-to-silence transitions. We worked around
it with `chunked_get_segments`, which slices the audio into 30 s pieces and
calls `get_segments` per chunk. This narrowed the bug's blast radius from
"the entire file" to "any 30 s chunk that opens with continuous speech," but
roughly half of the chunks on a typical AUOHP interview still return zero
segments because the bug recurses inside each chunk.

We want full segmentation recall and we don't want to fork the crate.

## 2. Proposal

Replace the `pyannote-rs` segmentation surface with our own thin module that
calls `ort` (the ONNX Runtime crate, already a transitive dependency) directly.
We keep `pyannote-rs`'s `EmbeddingExtractor` for now; only the `get_segments`
call goes away.

New module: `src/transcription/segmentation.rs`. New struct: `Segmenter`,
owning the `ort::Session` and the model's frame parameters.

## 3. What we keep from pyannote-rs

The model invocation pattern is correct; only the iterator bookkeeping is
wrong. We lift these pieces verbatim:

- 10 s sliding window over `i16` samples at 16 kHz
- Trailing zero-padding so the final window is full-length
- ONNX input shape: `[1, 1, samples]`, dtype f32 (cast from i16)
- Output tensor lookup by name `"output"`
- Per-frame argmax over the class axis: class 0 = silence, anything else =
  speech
- Frame-stride bookkeeping: the model emits one classification every
  `frame_size` samples, with the first frame anchored at `frame_start`

## 4. What changes

### 4.1 Bug fix: end-of-stream flush

After the final window is processed, if `is_speeching` is still true, emit a
closing segment that ends at the current `offset`. pyannote-rs never does
this, so any speech run that reaches the end of the input is lost.

### 4.2 Bug fix: don't terminate when the queue is momentarily empty

pyannote-rs's `from_fn` closure processes one window per call and returns
`pop_front()` immediately. If a window produces zero transitions the iterator
returns `None` and consumers (`.collect()`, `for ... in`) treat that as
end-of-stream, dropping every subsequent window.

The fix is to wrap the per-window processing in a `while
segments_queue.is_empty()` loop, so we keep advancing through windows until
either the queue has something to yield or the window iterator is exhausted.

Concretely, rather than `from_fn` we expose a single `Segmenter::segment(&mut
self, samples: &[i16]) -> Result<Vec<Segment>>` that runs to completion and
returns the full vector. We don't actually need streaming semantics --- the
caller already collects everything into memory --- and the eager API is much
harder to misuse.

### 4.3 Magic numbers become named constants

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

### 4.4 Session lifecycle

`Segmenter::new(model_path: &Path)` builds a single `ort::Session` and stores
it. Subsequent `segment()` calls reuse it. pyannote-rs already does this, but
because we're forced to reinstantiate `pyannote-rs` per chunk in the current
workaround, we end up paying session-build cost (model parsing + graph
optimization) on every chunk. The native implementation pays it once.

## 5. What we don't change yet

- `EmbeddingExtractor` and the wespeaker ONNX model stay as `pyannote-rs`
  imports. The embedding path doesn't have the same iterator bug --- it's a
  simple `compute(&samples)` call per segment --- and the wespeaker FBank
  feature extractor (`kaldi-native-fbank` integration) is non-trivial to
  replicate. Defer.
- The clustering and merge code in `diarize.rs` (Phases 2--3) is correct and
  reused unchanged.
- `pyannote-rs` stays in `Cargo.toml` for the embedding path. Once we
  replicate that too we can drop the dep entirely.

## 6. Migration

1. Add `src/transcription/segmentation.rs` with `Segmenter` and the bug-fixed
   loop.
2. In `diarize.rs`:
   - Replace the `chunked_get_segments` call with `Segmenter::segment`.
   - Convert the returned `Segment`s into the `(start, end, samples)` tuples
     the existing code expects --- or change the local type to match
     `pyannote_rs::Segment`'s shape directly. The downstream `EmbeddingExtractor::compute(&seg.samples)`
     call needs the i16 samples for that segment, so we keep the same fields.
3. Delete `chunked_get_segments`.
4. Verify on `/scratch/audio/026_iris_long_short.wav`: expect substantially
   more than 22 segments, with no chunk-boundary gaps.

## 7. Risk

The model output tensor layout (`[batch, time_frames, classes]`) is what
pyannote-rs assumes, and we'd be assuming the same. If the ONNX export ever
changes shape, both implementations break the same way. Low risk, high blast
radius.

The `frame_size`/`frame_start` constants are empirically tuned to the
specific ONNX export. If we accidentally pull a different segmentation model
into `models/`, our segment timestamps will drift. Mitigate by checksumming
the model file at `Segmenter::new` and refusing to load unexpected hashes.

## 8. Future: replace `EmbeddingExtractor` too

Once segmentation is native, the wespeaker embedding extractor is the only
remaining `pyannote-rs` surface. Replicating it requires:

- FBank feature extraction (use `kaldi-native-fbank` directly --- pyannote-rs
  already pulls it in)
- ONNX session for the wespeaker model
- L2-normalization of the output embedding

That's ~50 lines and unblocks dropping `pyannote-rs` entirely. Defer until
the segmentation rewrite is proven.
