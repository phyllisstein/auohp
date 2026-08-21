# The transcription tuning team

Four agents in `.claude/agents/`, plus a scoring harness, arranged around one
hard constraint.

## The constraint

**The machine has one NVIDIA L4.** A `large-v3` run with beam search and DTW over
34 minutes of audio takes minutes of wall clock and wants the whole card. Fanning
out N agents that each shell out to `cargo run --bin transcribe` produces
contention and OOM, not throughput.

So the team is deliberately **asymmetric** rather than a symmetric swarm: one
agent owns the GPU and serialises every run behind a `flock`; everyone else is
CPU-only and works on the *previous* run's artifacts while the next one executes.
Throughput is bounded by the runner, and analysis is free.

```
                     ┌─────────────────────┐
   config proposal   │   whisper-runner    │   run artifacts
  ───────────────────▶  (GPU, serialised)  ├──────────────────┐
        ▲            │   flock + manifest  │                  │
        │            └─────────────────────┘                  ▼
  ┌─────┴────────┐                                  ┌───────────────────┐
  │ whisper-tuner│◀─────── error taxonomy ──────────│ transcript-analyst│
  │  (ledger)    │                                  │   (CPU, parallel) │
  └──────────────┘                                  └───────────────────┘
                                                              │
                                                    uses ─────┘
                                                  ┌────────────────┐
                                                  │  score binary  │◀── eval-harness
                                                  └────────────────┘
```

| Agent | Model | Owns | Never |
|---|---|---|---|
| `whisper-runner` | haiku | GPU execution, run artifacts | Interprets results |
| `transcript-analyst` | opus | Error taxonomy, truth cleaning | Runs transcriptions |
| `whisper-tuner` | opus | Next config, the ledger | Runs or diagnoses |
| `eval-harness` | opus | `src/eval/`, `bin/score.rs` | Touches `src/transcription/` |

The last column is the load-bearing one. `eval-harness` is barred from the
pipeline and `whisper-runner` is barred from editing source for the same reason:
an agent that can adjust both the instrument and its subject can produce any
number it likes.

Code review uses the repo's existing `/code-review` and `/simplify` skills rather
than a fifth agent.

## Reading the numbers

**WER is not a grade.** `108_truth.txt` is OCR'd from a PDF and editorially
cleaned — disfluencies silently removed, page furniture interleaved mid-sentence,
scan damage throughout (`hi erar chai` for "hierarchical", `88:` where the speaker
tag should read `SS:`). A flawless transcription still scores poorly against it.
Every figure is a *relative* signal between configs on the same fixture.

The scorecard reports `ins_filler` separately from `ins_content` for this reason:
most insertions are disfluencies the transcriber dropped, not hallucinations, and
only the second bucket is worth reacting to.

**Check `anchor_confidence` first.** Near zero means the truth and hypothesis did
not align and nothing else on the card means anything.

**Rank by lexicon recall and substitutions, not raw WER.** The baselines showed
the WAV run scoring the *worst* WER and the *best* lexicon recall, with its entire
WER penalty coming from insertions while substitutions stayed flat. Insertions
here measure the transcriber's editing, not the model's accuracy.

**Respect the noise floor.** Two decodes correlating at 0.998 with zero drift
still differed by 8 points of lexicon recall. A delta under ~5 points needs a
repeat before it counts as a finding.

## Fixtures

| Fixture | Suffix | What it is |
|---|---|---|
| `108_funky.mp4` | `-original` | The **original** master. `mp4v` video, AAC-LC 44.1 kHz stereo. Production-representative; the tuning target. |
| `108_truth.mp4` | `-reencode` | H.264 re-encode of the original; audio carries an extra AAC generation. Control. |
| `108_truth.wav` | `-wav` | 16 kHz mono, externally resampled. Bypasses `mix_to_mono` and `resample`. Control. |

The `truth`/`funky` names record the order the fixtures were created, not their
fidelity — `funky` is the good one. Run ids use `original`/`reencode` so the
inversion does not propagate into every future comparison.

Media lives outside the repo (~1.2 GB). Set `AUOHP_FIXTURE_DIR`; tests needing
media skip with a notice when it is unset.

**Never compare across fixtures.** The pairwise gaps are themselves an
experiment: `-original` vs `-wav` prices our resampler against an external one,
and `-original` vs `-reencode` prices a generation of lossy re-encoding.

## Why correctness came before tuning

Four defects were visible in shipped output (`044_marlene_mccarty.json`) and a
fifth in the fixture containers. Two of them — DTW timings being read from the
wrong field, and control tokens leaking into word text — changed word text and
word timing wholesale. Parameter search run on top of those would have been
fitting noise.

The audio path gets settled before any decoder parameter is interpreted, for the
same reason: an audio defect degrades every metric simultaneously and will
masquerade as a dozen unrelated decoder problems. It is diagnosable in
`packages/core/tests/decode.rs` on CPU, with no GPU cost.

## Running it

```bash
export AUOHP_FIXTURE_DIR=~
export PATH=/usr/local/cuda-12.9/bin:$PATH
export LD_LIBRARY_PATH=/usr/local/cuda-12.9/lib64:$LD_LIBRARY_PATH

cargo test -p auohp-core --features cuda        # harness + audio path, no GPU
cargo build --release --features cuda --bin transcribe --bin score
```

Then drive the loop: tuner proposes → runner executes under `flock` → scorer
scores → analyst reports → ledger row appended in
[`transcription-tuning-log.md`](./transcription-tuning-log.md).
