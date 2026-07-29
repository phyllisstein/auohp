# Transcription tuning ledger

Append-only. One row per GPU run. Never rewrite history and never delete a row
because it turned out uninteresting — **negative results are the most valuable
entries here**, because they are what stops the campaign from rediscovering the
same dead end.

Maintained by `whisper-tuner`. Runs executed by `whisper-runner`. Diagnoses by
`transcript-analyst`.

## Interviews

| Id | Subject | Interviewer | Clip | Turns | Character |
|---|---|---|---|---|---|
| 108 | Avram Finkelstein | Sarah Schulman | 34:00 | 64 | Long reflective answers; art/organising vocabulary. |
| 026 | Iris Long | Sarah Schulman | 24:45 | 174 | Dense clinical and regulatory vocabulary (DHPG, CMV retinitis, ACTG, parallel track). Nearly 3× the turn density of 108, so a much harder diarization test. |
| 074 | Douglas Crimp | Sarah Schulman | 16:54 | 56 | Graphics, culture and NYC politics. Shortest clip and truth (2003 tokens), but full coverage and the densest lexicon (29 terms in 2003 tokens). Its transcript needed no layout reconstruction — tags are inline. |

### A note on the 026 transcript reconstruction

026's transcript came from `pdftotext` over a Word-authored PDF whose speaker tag
sits in a left column and text in a right one. The extractor emits each turn's
*first* line in reading order but defers the wrapped remainder, so continuations
resurface later — sometimes after an intervening speaker:

```
SS:  How did ACT UP respond to this information? What did they decide
IL:  They essentially accepted it and put it on their agenda. That was it. It was
     to do?          <- belongs to SS
     on the agenda.  <- belongs to IL
```

Two signals recover the original order, and `scripts` note them because getting
this wrong would silently corrupt the diarization metric rather than erroring:

- **A blank line separates layout blocks.** Blank-separated text that is not the
  line immediately after a tag is a *displaced* continuation.
- **Contiguous lines are ordinary wrapping** and stay where they are.

Displaced lines refill the oldest turn still ending mid-sentence — FIFO. That one
rule resolved all 84 displacements in the file, including `names.` landing back
on a turn three exchanges earlier. The builder asserts that concatenating the
turns reproduces the cleaned truth token-for-token, so the two fixtures cannot
drift apart.

## How to read these numbers

**WER is not a grade.** Both transcripts are PDF extractions, editorially cleaned,
with disfluencies silently removed. A flawless transcription still scores poorly.
Every figure here is a *relative* signal between configs on the same fixture.

**Read `sub` before `wer` before `lex`.** Substitutions are computed over
thousands of tokens and are the only ranking that has replicated across two
interviews. WER is dominated by insertions, which measure the transcriber's
editing habits. Lexicon recall has a denominator of 76–163 occurrences and swings
accordingly.

**Insertions are the editorial surface, not a defect to close.** See "Settled:
no post-processing" below before proposing anything that acts on them.

**Check `anchor_confidence` and `partial_coverage` first.** Low confidence means
the streams did not align and nothing else on the card means anything; partial
coverage means the numbers describe an overlap, not the whole file.

**Fixtures are not interchangeable** — not even between interviews with the same
suffix.

| Fixture | Suffix | What it is |
|---|---|---|
| `108_funky.mp4` | `-original` | The **original** master: `mp4v` video, AAC-LC 44.1 kHz stereo. Production-representative. |
| `108_truth.mp4` | `-reencode` | H.264 re-encode; audio carries an extra AAC generation. |
| `108_truth.wav` | `-wav` | 16 kHz mono, extracted **from the original**. |
| `026_original.mp4` | `-original` | `avc1` H.264, AAC-LC 44.1 kHz stereo. |
| `026_reencoded.mp4` | `-reencode` | Also H.264, and *larger* than the original — not a lossy transcode. |
| `026_audio.wav` | `-wav` | 16 kHz mono, extracted **from the re-encode**, not the original. |

108's `truth`/`funky` names record the order they were created, not their
fidelity; `funky` is the original. And note the last row: the two WAVs have
different parents, so `-wav` does not mean the same thing in the two interviews.

## Harness version

Scores are comparable only within a `harness_version`. When `eval-harness` bumps
it, archived `result.json` files are re-scored (CPU-only, no GPU cost) and the
boundary is marked below.

Currently: **v4**. All runs below were re-scored at each bump, so the table is
internally consistent.

- v1 → v2: symbols spoken as words (`=`, `&`, `%`, `+`) normalise to those words
  instead of being stripped; lexicon entries that normalise alike are collapsed.
- v2 → v3: contraction clitics that lost their apostrophe are reattached. Whisper
  intermittently emits `wouldn t` / `I m` where the truth has `wouldn’t` / `I’m`,
  which previously produced a spurious substitution *and* insertion apiece. This
  cut `000-reencode`'s substitution count from 50 to 37 — a quarter of its
  substitutions were the instrument's fault, not the model's.
- v3 → v4: added `StructureStats::speaker` (segment boundaries vs speaker turns)
  and `partial_coverage`. `anchor` now estimates each end from whichever stream
  actually has covered material there, instead of always voting on the truth.
  Without that, a transcript covering more tape than its clip made the truth's
  tail vote pure noise, which collapsed the aligned window to a sixth of the real
  overlap and reported ~4 050 phantom deletions on 026. 108's scores are byte
  identical either way, since it has full coverage.

---

## Runs

| Run | Parent | Fixture | Axis changed | WER | Lex recall | Verdict |
|---|---|---|---|---|---|---|
| 000-original | — | `108_funky.mp4` | baseline after Phase 1 fixes | 0.0996 | 0.8816 | reference |
| 000-reencode | — | `108_truth.mp4` | control | **0.0933** | 0.9211 | fewest subs |
| 000-wav | — | `108_truth.wav` | control: externally resampled | 0.1410 | 0.9605 | worst WER |
| 001-026-original | — | `026_original.mp4` | second interview, baseline | 0.3018 | 0.8592 | reference |
| 001-026-reencode | — | `026_reencoded.mp4` | control | 0.3125 | 0.9286 | fewest subs |
| 001-026-wav | — | `026_audio.wav` | control: from the *re-encode* | 0.2783 | 0.8857 | best WER |
| 002-074-original | — | `074_original.mp4` | third interview, baseline | 0.2454 | 0.9000 | reference |
| 002-074-reencode | — | `074_reencoded.mp4` | control | **0.1434** | 0.8500 | subs tied |

Full breakdown (harness v4). **Read `sub` first** — see the correction below.

| Run | WER | **sub** | del | ins filler | ins content | lex recall | ctrl | segs | seg len | boundary lift | bleed | wall |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 000-original | 0.0996 | 63 | 31 | 91 | 272 | 0.8816 | **0** | 417 | 12.0 | 8.96× | 16 | 261 s |
| 000-reencode | **0.0933** | **37** | 32 | 83 | 276 | 0.9211 | **0** | 503 | 9.9 | 8.99× | 6 | 385 s |
| 000-wav | 0.1410 | 62 | 30 | 94 | **461** | 0.9605 | **0** | 675 | 7.7 | 7.43× | 2 | 262 s |
| 001-026-original | 0.3018 | 121 | 94 | 106 | 338 | 0.8592 | **0** | 301 | 10.8 | 10.24× | 6 | 184 s |
| 001-026-reencode | 0.3125 | **55** | 75 | 118 | 406 | 0.9286 | **0** | 252 | 13.4 | **10.81×** | 21 | 217 s |
| 001-026-wav | **0.2783** | 61 | 81 | 104 | 336 | 0.8857 | **0** | 340 | 9.6 | 9.53× | **1** | 205 s |
| 002-074-original | 0.2454 | 32 | 29 | 96 | 315 | 0.9000 | **0** | 186 | 11.5 | 9.32× | 16 | 142 s |
| 002-074-reencode | **0.1434** | 31 | 52 | 61 | 103 | 0.8500 | **0** | 170 | 11.3 | 8.16× | 20 | 96 s |

026 is scored on **partial coverage**: its transcript covers roughly twice the
tape the clip does (Tape I 00:35 → Tape II 00:35, ~40 min, against a 24:45 clip).
The scorecard reports this and scores only the overlap — `truth[0..2396]` against
`hyp[454..3264]`.

### What replicated across two interviews, and what did not

| Claim | 108 | 026 | 074 | Verdict |
|---|---|---|---|---|
| Segment boundaries track speaker changes | 7.4–9.0× | 9.5–10.8× | 8.2–9.3× | **Replicates 3/3** |
| Re-encode has the fewest substitutions | 37 vs 63 | 55 vs 121 | 31 vs 32 (tied) | Holds 2/3, tied on the third |
| Re-encode has the best lexicon recall | no | yes | no | Does not replicate |
| Lowest WER identifies the best input | re-encode | WAV | re-encode | Does not replicate |

**Only the boundary-lift finding is unqualified.** The substitution advantage is
real on 108 and 026 and absent on 074 — a tie, not a reversal, so it is not
contradicted, but "the re-encode is always better" is not supported. 074's
re-encode wins on WER by a wide margin (0.1434 vs 0.2454) and that is entirely
insertions: 103 against 315, with *more* deletions (52 vs 29). It is a terser
transcription, not a more accurate one.

Lexicon recall now points a different way in each of the three interviews, which
is what a 60–180 occurrence denominator does.

---

## Round 1 — Whisper parameter tuning

Plan: [`transcription-tuning-round1-plan.md`](./transcription-tuning-round1-plan.md),
designed by `whisper-tuner` from the ledger **and the vendored whisper.cpp source**.
Three axes were closed by source inspection before any GPU time was spent:

- **`patience` is a dead knob.** It appears twice in `whisper.cpp`, both as default
  initialisers, and is never read by the decode loop.
- **`split_on_word` alone is a no-op** — it requires `max_len > 0`
  (`whisper.cpp:6059`), which in turn requires `token_timestamps`
  (`whisper.cpp:7649`).
- **`max_len`/`split_on_word` cannot change a single token.** `whisper_wrap_segment`
  is post-processing over an already-decoded token list, so those axes can only
  move `structure.*`.

### 003-r0 — determinism, settled for one run

A verbatim repeat of `000-original`. The result matters more than any single
parameter, because it decides whether every later run needs a repeat.

| | 000-original | 003-r0 |
|---|---|---|
| segment text | — | **byte-identical**, all 417 segments |
| word text | — | **byte-identical**, all 4 936 words |
| WER / sub / del / insC | 0.0996 / 63 / 31 / 272 | identical |
| words differing at all | — | **2 of 4 936**, DTW timing only, one 20 ms boundary |

**Decoding is deterministic in text.** So:

- No config ever needs repeating. Every run buys clean signal.
- Any delta in `subs`, WER, insertions, deletions or lexicon recall is **real**,
  not noise. There is no config-level noise floor on this fixture.
- The ledger's noise-floor caveat applies **only across fixtures**, never across
  configs on one fixture.

The one caveat that survives, and it is the tuner's point rather than mine:
determinism removes *measurement* noise, not *sensitivity*. The baseline's 63
substitutions are a long tail of singletons, so a config change that shifts one
VAD boundary by 10 ms can chaotically re-roll a dozen unrelated tokens. A
deterministic 5-substitution delta is a real difference between two configs but
not necessarily an attributable effect of the axis. The defence is monotone
dose-response across three or more rungs of the same axis — which is why the plan
is built from ladders rather than single probes — and repeats would not have
helped with it at all.

### CORRECTION: VAD has never run

Seven round-1 experiments varying `vad.threshold` (0.35/0.65), `speech_pad_ms`
(100/250), `min_silence_duration_ms` (300/700), `max_speech_duration_s` (120) and
`samples_overlap_s` (0.0) all returned metrics **byte-identical to the parent**.
That range producing zero change is a symptom, not a null result.

`WhisperState::full()` calls `whisper_full_with_state`
(`whisper-rs-0.16.0/src/whisper_state/mod.rs:299`). The `params.vad` check exists
only in `whisper_full` (`whisper.cpp:7743`) and `whisper_full_parallel` (7766);
`whisper_full_with_state` never reads it. So `enable_vad(true)`, the silero model
path, and `max_speech_duration_s: 60.0` have been **inert in every run ever
made**, including all baselines.

Two claims elsewhere in this ledger were wrong and are corrected here:

- The ~9× speaker-boundary lift is **not** silero finding conversational pauses.
  It is Whisper's own timestamp-token segmentation, with no VAD in the pipeline
  at all. This makes the diarization prospect *better*: the boundaries come free
  from the decoder and need no second model.
- The speculation that segmentation improved because "VAD was previously
  operating on interleaved stereo misread as mono" is unfounded. The channel-count
  bug was real and its fix was real, but VAD played no part in either.

All VAD axes are unavailable until this is addressed. whisper-rs exposes a
separate `WhisperVadContext`, so the fix is to run VAD explicitly and feed it
filtered samples — a design decision, not a bug fix, since the pipeline
demonstrably works without it.

### The entropy gate — the round's main result

`entropy_thold` ships at **3.0**; whisper.cpp's default is **2.4**
(`whisper.cpp:5954`). The value is Shannon entropy over the multiset of the last
32 tokens (`whisper.cpp:6584-6604`), so its ceiling is `ln 32 = 3.466`. When a
window falls below the threshold it is discarded and re-decoded up the
temperature ladder (`whisper.cpp:7527`) — and the ladder is live, because
`temperature_inc: None` takes whisper.cpp's 0.2 default.

Measured on 108, deterministic so every delta is real:

| Config | WER | **sub** | del | ins content | hyp tokens | lift | anchor conf |
|---|---|---|---|---|---|---|---|
| `temperature_inc` 0.0 (fallback off) | 1.8697 | 400 | 10 | 6569 | **12 749** | 5.58 | **14** |
| `entropy_thold` 2.4 (whisper.cpp stock) | 0.1251 | 111 | 25 | 337 | 5002 | 8.66 | 29 |
| `entropy_thold` 3.0 (**shipped baseline**) | 0.0996 | 63 | 31 | 272 | 4921 | 8.96 | 41 |
| `entropy_thold` **3.4** | **0.0617** | **40** | 29 | **182** | 4774 | **9.94** | 44 |

Monotone across four points. `3.4` cuts substitutions 37 % and WER 38 % relative,
and *improves* speaker-boundary lift as a side effect.

**The plan's hypothesis was backwards, and so was an earlier entry in this
ledger.** The prediction was that 3.0 fires spuriously on ordinary prose and that
lowering it to stock 2.4 would help. Lowering it made things clearly worse, and
`A4` — included only as a deliberate "predicted much worse" positive control —
turned out to be the best configuration found. A single-probe design would have
tested 2.4, seen degradation, and closed the axis as a dead end.

#### What the gate actually does

Two distinct mechanisms, both verified in source:

1. **Per-candidate pruning** (`whisper.cpp:7517-7536`). The gate runs inside a
   loop over beam-search *candidates*. A candidate whose last-32-token entropy
   falls below the threshold is marked `failed` and `continue`d — skipped in the
   `best_score` comparison. So it prunes repetitive candidates from selection,
   and a **higher** threshold prunes more aggressively. This is what produces the
   monotone gain.
2. **Window retry** (`whisper.cpp:7548-7560`). Only if the *selected best*
   candidate is itself failed, and only when not already at the last temperature,
   is the whole window re-decoded one rung up the ladder.

`temperature_inc: 0.0` collapses `temperatures` to a single rung, so the retry can
never fire and a repetitive result is accepted verbatim. Its output is textbook:

> realized that that was realized that that was realized that that was you
> couldn't have the one without the you couldn't have the one without the other
> other other and it's the mayo if they're it's the and it's the mayo…

So the false-start fragments Daniel identified by eye in `000-wav` were repetition
the gate **failed to catch** — not damage the gate caused.

#### The ladder saturates at ~3.4, and it validates on held-back interviews

| Interview | entropy | WER | **sub** | del | ins content | lex | anchor conf |
|---|---|---|---|---|---|---|---|
| 108 | 2.4 | 0.1251 | 111 | 25 | 337 | 0.9157 | 29 |
| 108 | 3.0 (baseline) | 0.0996 | 63 | 31 | 272 | 0.8916 | 41 |
| 108 | 3.2 | 0.0900 | 44 | 31 | 290 | 0.9518 | 58 |
| 108 | **3.4** | **0.0617** | 40 | 29 | 182 | 0.9277 | 44 |
| 108 | 3.45 | 0.0621 | **37** | 31 | 178 | 0.9398 | 44 |
| 108 | 3.46 | 0.0621 | **37** | 31 | 178 | 0.9398 | 44 |
| 026 | 3.0 (baseline) | 0.3018 | 121 | 94 | 338 | 0.8765 | 17 |
| 026 | **3.4** | **0.2228** | **58** | 71 | 267 | 0.9000 | 10 |
| 074 | 3.0 (baseline) | 0.2454 | 32 | 29 | 315 | 0.9000 | 48 |
| 074 | **3.4** | **0.0941** | **23** | 38 | 60 | 0.9167 | 52 |

3.45 and 3.46 are byte-identical, so the axis is saturated past ~3.4 — expected,
since the gate's ceiling is `ln 32 = 3.4657`.

**Substitutions fall on all three interviews: −37 % (108), −52 % (026), −28 %
(074).** WER falls −38 %, −26 %, −62 %. This is the first change in the campaign
that generalises beyond its tuning fixture.

**Recommended default: `entropy_thold: 3.4`.** Preferred over 3.45 because it is
the value validated on all three interviews rather than on 108 alone, and because
it is not perched within 0.02 of the gate's ceiling.

Two things to hold onto:

- **026's `anchor_confidence` fell 17 → 10** while its scores improved. That is
  the borderline of usability, and 026 is already scored on partial coverage. The
  effect size (substitutions halved) is far larger than the alignment wobble, but
  the exact figures on that row are the least trustworthy in the table.
- **074 got terser, not merely better**: deletions rose 29 → 38 while content
  insertions collapsed 315 → 60. Most of its WER gain is insertions it stopped
  making. Given that insertions on this corpus largely track the transcriber's
  editing rather than model error, read 074's −62 % WER as flattering.

Lexicon recall is non-monotone across the ladder (0.892 → 0.952 → 0.928 → 0.940)
— exactly the small-denominator behaviour this ledger warns about at 83 term
occurrences. Do not read it as an ordering.

#### Caveat on two of these rows

`anchor_confidence` degrades as output quality does: 14 for the `temperature_inc`
run and 29 for `entropy_thold` 2.4, against 41 at baseline. Runaway repetition
breaks the alignment that the metrics are computed on, so those two rows are
directionally certain but numerically loose. The 3.4 row is the opposite — its
confidence *rose* to 44.

#### Other axes

| Axis | Result |
|---|---|
| `no_context` → false | Byte-identical to baseline. Config verified applied; a genuine null with no explanation yet. |
| `beam_size` → 2 | Worse: WER 0.1325, and though subs fall to 51, deletions double (56 vs 31) and insertions rise to 421. |
| `max_len` → 80 | Segments 417 → 554, boundary lift 8.96× → 6.75×. Text essentially unchanged (subs 67 vs 63) as predicted for post-processing. |

---

## ⚠ Comparability boundary: run `029-vad-on` onwards decode different audio

Every run numbered **000–028 decoded without VAD**. Not by choice — `params.vad`
was never read on this code path (see the code items below), so silero was inert
regardless of what the config said. `029-vad-on` is the first run in this
pipeline's history where speech detection actually ran.

On 108 it drops 13 % of the audio: 632 speech regions, 1771.8 s kept of 2040.8 s.

**Do not compare across this line on anything timing- or segmentation-related.**
Specifically invalid across the boundary:

- `structure.segments`, `median_segment_seconds`, `boundary_quantization`
- `structure.speaker.*` — including the ~9× lift, which was measured on
  Whisper's own timestamp tokens with no VAD present
- every word and segment timestamp, and the tape-anchor drift fit
- caption-shape statistics (character and duration distributions)

Text metrics (`subs`, `dels`, WER, lexicon recall) are *probably* comparable but
not guaranteed: removing 13 % of the audio changes what each decode window
contains, so window boundaries fall elsewhere and tokens can re-roll.

**Before treating round 1's table as current, re-baseline `entropy_thold: 3.4`
with VAD enabled.** Until that exists, the recommended default rests on
measurements taken in a configuration the pipeline no longer runs.

The timestamps themselves are only meaningful because `apply_vad` returns a
`VadTimeline` and every segment and word time is mapped back to the real
recording before leaving `whisper.rs`. Filtered-timeline times would look
entirely plausible in the JSON and drift further out of sync the more silence a
recording contains.

---

## Round 2 — the decode block is exhausted

Parent `P1` = `006-a4` (`entropy_thold: 3.4`), on `108_funky.mp4`.

| Run | Axis | WER | **sub** | del | ins content | lex | anchor conf | lift |
|---|---|---|---|---|---|---|---|---|
| `006-a4` | **P1 baseline** | **0.0617** | **40** | 29 | 182 | 0.9277 | 44 | 9.94 |
| `023-n1` | `logprob_thold` −0.5 | 0.0617 | 40 | 29 | 182 | 0.9277 | 44 | 9.94 |
| `028-h1` | `no_speech_thold` 0.2 | 0.0617 | 40 | 29 | 182 | 0.9277 | 44 | 9.94 |
| `024-l1` | `temperature_inc` 0.1 | 0.0630 | 42 | 30 | 182 | 0.9398 | 66 | 9.19 |
| `026-l2` | `temperature_inc` 0.4 | 0.0636 | 39 | 31 | 184 | 0.9277 | 66 | 9.55 |
| `025-b8` | `beam_size` 8 | 0.0691 | 41 | **45** | 187 | 0.9398 | 65 | 10.06 |
| `027-b2` | `beam_size` 2 | 0.0656 | 45 | 37 | 178 | 0.9157 | 41 | 9.66 |

**Nothing beats P1.** The best round-2 substitution count is 39 against 40 — a
one-token difference on a metric whose baseline is a long tail of singletons,
which is precisely the "deterministic but not attributable" band this ledger
warns about. There is no round-2 winner to validate.

### The two nulls are masked, not inert — and that is now settled

`logprob_thold` and `no_speech_thold` both returned **byte-identical** output.
That is the confound the round-2 plan predicted in advance: the retry condition
at `whisper.cpp:7554` is

```c
decoder.failed || (avg_logprobs < logprob_thold && no_speech_prob < no_speech_thold)
```

At `entropy_thold: 3.4` the `failed` disjunct already fires on nearly every
window, so the right-hand clause can never change the outcome. Both axes are
**unreachable at the winning config**, not proven inert on this corpus. The `N4`
control (same axis, parent `000-original` at 3.0) would separate the two, but its
only use would be explanatory: neither axis can improve on P1 while P1 is the
config we ship.

### `beam_size` interacts with the entropy gate, as predicted

Round 1 measured `beam_size: 2` at `entropy_thold: 3.0` and got WER 0.1325, with
deletions 56 and insertions 421. At 3.4 the same setting gives WER 0.0656,
deletions 37, insertions 178 — **much less bad**. The gate compensates for a
narrow beam, so the two axes are mechanically coupled and round 1's beam result
did not transfer. Widening instead (beam 8) is also worse than P1, and costs
deletions (45 vs 29). Beam width closes at 5.

### The retry-rate proxy did not survive

The plan used wall clock as the observable stand-in for retry count, since
whisper.cpp's `fallbacks = p / h` counters are unreachable through this API.
That measurement is **not usable from this batch**: the runs were launched as
parallel drivers serialised on a `flock`, so elapsed time between writing
`config.json` and `result.json` is dominated by lock waiting, not decoding. Any
future round needing this must time the `flock`ed invocation itself, from inside
the critical section.

### Verdict

The round-2 plan set a stopping condition before any of this ran: if the retry-path
thresholds and rung density all came back null, the decode block is exhausted and
GPU time should go elsewhere. That condition is met — the only untested axis is
`n_max_text_ctx`, which needs a config field.

**Recommendation: stop sweeping decode parameters.** Ship `entropy_thold: 3.4`
and spend effort on the code items below, none of which need the card.

---

## Settled: no post-processing — 2026-07-29

**Decided by Daniel. Do not re-propose a filler filter, a repair detector, or any
other cleanup pass over the emitted word stream.**

The bar for this pipeline is *"not broken-looking, and clear enough to be
human-refined."* The video path now clears it, and everything past that point is
editorial correction, done by a person, downstream.

The temptation to reopen this is strong and specific, so the reasoning is recorded
rather than the conclusion alone. In the passage below — the one that first showed
the WAV/MP4 gap — the human transcript removes all of this:

```
as with any, in any            ->  as in any
that that was -- you couldn't  ->  you couldn't
And it's the -- it was the myopia  ->  It was the myopia
```

None of it is `um`, `uh`, or `you know`. They are **repairs**: aborted attempt,
interruption, retry, with the transcriber keeping the retry. So a filter aimed at
this tier is not a stopword list, it is repair detection — and a repair detector
that is 90% right emits *confidently wrong* text, which is strictly worse to hand
an editor than visible disfluency, because the editor can no longer see what was
removed. Visible disfluency is self-announcing; a bad repair is not.

Two consequences for reading the ledger:

- `ins_content` (179 at `029-vad-on`) is **not** a defect with headroom. It is real
  speech that the transcriber edited away, and it is the surface an editor works
  on. Do not rank configs by it.
- WER is therefore mostly not-error, which is why `sub` leads the ranking. This is
  the concrete reason behind "read `sub` before `wer` before `lex`."

Separately, and worth not confusing with the above: the WAV's
`It's the myo... If there... It's the myo...` was **not** disfluency at all — it is
a decoder repetition loop on a truncated word, i.e. speech nobody produced. That
class *is* ours to fix, it is what the entropy gate prunes, and
`entropy_thold: 3.4` is the fix already in place.

## Open code items — both worth more than further threshold sweeps

Neither needs the GPU. Both were found by reading whisper.cpp during round 2, and
both gate axes the campaign currently cannot reach.

### 1. `carry_initial_prompt` is not exposed — the domain prompt reaches ~60 s

`whisper_full_params.carry_initial_prompt` exists in the bindings
(`whisper-rs-sys-0.15.0/src/bindings.rs:5456`) but whisper-rs 0.16 never sets it
and provides no setter. So `set_initial_prompt` takes the `else` branch at
`whisper.cpp:6939`, pushing the prompt into `prompt_past1` — the **rolling**
buffer. The take at `7106` keeps only the last `max_prompt_ctx - 1` = 223 tokens
(`min(n_max_text_ctx, n_text_ctx/2)` for large-v3), so once ~223 tokens have been
decoded the prompt is evicted from the front. That is roughly two decode windows:
**the prompt primes the first minute of a 34-minute interview and then vanishes.**

Consequence: **the assumption that `set_initial_prompt` improves whole-interview
quality is not currently true.** It is the archive's highest-leverage axis for
proper nouns and terms of art, and it is nearly inert as wired.

Routes, in order of preference:

| Route | Cost | Notes |
|---|---|---|
| Patch whisper-rs with one added method | ~10 lines, one vendored crate | `pub fn set_carry_initial_prompt(&mut self, v: bool) { self.fp.carry_initial_prompt = v; }` behind `[patch.crates-io]`. Upstreamable. |
| Build `whisper_full_params` via sys and call `whisper_full_with_state` directly | large | Requires raw `ctx`/`state` pointers, which whisper-rs does not expose either, so the context would have to be created through sys as well. Bypasses the safe wrapper for the whole inference path. |
| Transmute `FullParams` | — | `fp` is `pub(crate)`; the struct carries lifetimes and phantom data. Do not. |

### 2. `params.vad` is never read on this code path

`WhisperState::full()` calls `whisper_full_with_state`, and the `params.vad`
check exists only in `whisper_full` (`whisper.cpp:7743`) and
`whisper_full_parallel` (7766). Seven VAD axes are unreachable, and every run
this campaign has ever made — including all baselines — decoded without VAD.

whisper-rs does expose a standalone `WhisperVadContext` (`whisper_vad.rs`), so
the fix is to run VAD explicitly and pass filtered samples to `full()`. That is a
design decision rather than a bug fix: the pipeline demonstrably works without
VAD, and the ~9× speaker-boundary lift comes from Whisper's own segmentation.

### 3. Smaller: `n_max_text_ctx` is not in `TranscribeConfig`

whisper-rs exposes `set_n_max_text_ctx`; the config does not carry it. It is the
real context switch (`whisper.cpp:7090`, `7094`) that `no_context` was mistakenly
believed to be, and no run in this campaign has ever decoded without rolling
self-conditioning. Three lines.

### 4. Smaller: the filler classifier under-counts verbal tics

`you know` occurs 3× in the 108 truth and 26× in the hypothesis — ~46 inserted
tokens — but `ins_filler` is only 91 including `um`×12 and stutters. The
classifier matches filler phrases only against runs of *consecutive* insertions,
so when the model inserts `know` after a `you` that legitimately aligns, the pair
splits across ops and only `know` is counted, as content. Does not bias any
ranking (the campaign ranks on `subs`) but makes `ins_content` misleading in
absolute terms.

---

## Phase 0 — the audio path, settled on CPU

**No GPU time was spent on this.** `packages/core/tests/decode.rs` compares the
decoded sample streams directly, with Whisper out of the loop.

### The `audio.rs:122` FIXME was real, and it was not the resampler

The comment blamed "extracting audio from an MP4". The actual cause was the
**channel count**. Symphonia cannot recover a channel map from the `esds` of this
corpus's original masters, so `codec_params.channels` comes back `None` — and the
old code read that as `.unwrap_or(1)`. A stereo file was therefore treated as
mono: `mix_to_mono` never ran, and an interleaved L,R,L,R stream was handed to the
resampler as a single channel. Double length, both channels smeared together,
every timestamp wrong.

It failed silently and only on the original masters, which is why it survived as
folklore rather than being found. `decode_file_with` now takes the layout from the
decoded frames, which the decoder always knows because it just produced them.

Before the fix, `108_funky.mp4` did not decode at all under the corrected
track-selection predicate — the audio track was rejected for having no channel
count. That failure is what exposed the bug.

### The resampler is fine and is not worth tuning

Measured against the externally resampled WAV, after removing a constant global
offset:

| Comparison | mean NCC | residual drift | length delta |
|---|---|---|---|
| original vs WAV | **0.9978** | 0 samples, all 8 windows | 12 829 (0.04 %) |
| re-encode vs WAV | **0.9941** | 0 samples, all 8 windows | 569 |

Zero residual drift across the file rules out samples being dropped or duplicated
at chunk boundaries. `RESAMPLE_CHUNK`, `sinc_len`, `f_cutoff`, and
`SincInterpolationType` should be considered **settled** — do not spend runs on
them without new evidence.

### Generational loss is measurable, and the prediction held

The original (0.9978) matches the WAV better than the re-encode (0.9941) does,
consistent with the re-encode carrying one extra AAC generation. So the two MP4s
are **not** interchangeable, and transcoding before transcription is not a free
preprocessing step.

Global offsets: re-encode −788 samples (−49 ms), which is textbook AAC priming
(2112 @ 44.1 kHz → ~766 @ 16 kHz). The original's −12 927 samples (−808 ms) is a
genuine content offset — the master starts about 0.8 s earlier than the WAV
extract.

### WAV provenance

The WAV correlates with the original (0.9977) over the re-encode (0.9939), so it
was extracted from the master and is the cleanest available control.

---

## Notes per run

### 000-original / 000-reencode / 000-wav — baseline after Phase 1 fixes

**Phase 1 fixes confirmed.** `control_token_words` is 0 on all three, down from
3 410 in the archived pre-fix run of the same interview. `backwards_time_words`
is 0. All seven tape anchors now locate, against four before, and residuals
scatter (−34 s … +42 s) without a monotonic trend — placement noise in
hand-positioned PDF margin timecodes, not drift. `boundary_quantization` is
0.007, so segment boundaries follow speech rather than VAD windows.

#### CORRECTION: lexicon recall is the noisy metric; substitutions are the signal

An earlier revision of this section argued that WER was "dominated by the
transcriber's editorial habits" and that configs should be ranked by **lexicon
recall over WER**. That was wrong, and reading the transcripts side by side is
what exposed it.

The WAV's excess insertions are not faithful disfluency capture. They are
fragmentary false starts the model emits and then restarts:

> **WAV:** And it's the myo… If there… It's the myo… It was the myopia of ACT UP…
> **re-encode:** And it's the — it was the myopia of ACT UP…

> **WAV:** Every moment from the second ACT UP started until… until I completely burnt out
> **re-encode:** Every moment. From the second ACT UP started until I completely burnt out

That is a real quality defect, and **WER caught it while lexicon recall did not**.
Recall is blind to it by construction: repeating a fragment does not remove a term
from the transcript, and may even add one.

Why recall looked better on the worse input: its denominator is only **76 term
occurrences** in this interview, so a swing of six terms moves it eight points.
Substitutions are computed over 4 587 tokens and are correspondingly stable — 63,
37, 62 across the three runs, with the re-encode clearly ahead.

**Ranking rule: substitutions first, then WER, then lexicon recall.** Treat lexicon
recall as a diagnostic of *which* terms fail, not as a scalar for ranking configs;
its confusion list is the valuable part.

#### The noise floor is in the metric, not the model

The earlier claim that "Whisper is surprisingly sensitive to inaudible audio
differences" was the same mistake seen from another angle. `000-original` and
`000-wav` correlate at 0.998 and differ by 8 points of recall — but their
substitution counts are 63 and 62, i.e. essentially identical. The instability was
lexicon recall's small denominator, not the model.

**Revised guidance:** a lexicon-recall delta under ~8 points is within its own
sampling noise on a single interview and should not be reported as an effect. A
substitution-count delta of the size seen here (63 → 37, a quarter of the total)
is well outside noise and can be acted on.

#### The 026 fixtures are not the same experiment as 108's

Two differences change what the pair can test, and both were found by measuring
rather than assumed from the filenames:

- **Both 026 MP4s are `avc1` H.264**, and `026_reencoded.mp4` is *larger* than the
  original (420 MB vs 311 MB). It is not a lossy transcode, so this pair says
  nothing about generational loss.
- **026's WAV was extracted from the re-encode**, not the original — the decode
  test resolves this (ncc 0.99958 to the re-encode against 0.99882 to the
  original), where 108's WAV came from its original. So for 026 the re-encode
  correlates best with the WAV *because the WAV was made from it*. That is
  circular and must not be read as evidence of quality.

The resampler measures clean on 026 as well: original vs WAV **0.9989** mean NCC
with zero residual drift, against 0.9978 on 108.

#### Segment granularity trades off against speaker bleed

026 makes a pattern visible that 108 only hinted at. Ranked by mean segment
length:

| Run | seg len | bleed | lift |
|---|---|---|---|
| 001-026-reencode | 13.4 tok | 21 | 10.81× |
| 001-026-original | 10.8 tok | 6 | 10.24× |
| 001-026-wav | 9.6 tok | **1** | 9.53× |

Coarser segmentation means fewer, longer segments — and more chances for one of
them to straddle a speaker change. The re-encode has the *fewest* substitutions
and the *most* bleed simultaneously. So "which input is best" depends on what the
segment is for: text accuracy favours the re-encode, per-speaker attribution
favours finer segmentation. If diarization gets revived, that is a real tension to
settle deliberately rather than by accident, and `set_max_len` is the knob.

#### Segmentation is already doing most of the diarization work

Reported after Daniel noticed by eye that interviewer lines stopped bleeding into
subject lines. It holds, it is much stronger than "vibes", and it **replicates on
a second interview with nearly three times the turn density**:

| Run | segments | mean seg | changes on a boundary | expected by chance | **lift** | bleed |
|---|---|---|---|---|---|---|
| 000-original | 417 | 12.0 tok | 47/63 | 5.2 | **8.96×** | 16 |
| 000-reencode | 503 | 9.9 tok | **57/63** | 6.3 | **8.99×** | **6** |
| 000-wav | 675 | 7.7 tok | 61/63 | 8.2 | 7.43× | 2 |
| 001-026-original | 301 | 10.8 tok | 102/108 | 10.0 | **10.24×** | 6 |
| 001-026-reencode | 252 | 13.4 tok | 87/108 | 8.0 | **10.81×** | 21 |
| 001-026-wav | 340 | 9.6 tok | 106/107 | 11.1 | **9.53×** | **1** |

026 has 108 speaker changes in 24:45 against 108's 63 in 34:00, and the lift goes
*up* rather than down. Denser turn-taking gives silero more real pauses to find,
not fewer.

Segment boundaries fall on speaker changes roughly **nine times more often than
chance**. Silero VAD is cutting at conversational turn-taking pauses, which is
exactly where the speaker changes.

**Read `lift`, not `covered`.** Raw coverage is trivially gamed by segmenting more
finely — at one word per segment every change lands on a boundary. The WAV's 61/63
looks best but comes from 675 segments averaging 7.7 tokens, and its lift is the
*worst* of the three. The re-encode gets 57/63 at the same efficiency as the
original while cutting bleed from 16 to 6.

**Why this matters:** diarization was abandoned because boundary detection on this
corpus did not work. It now largely does, for free, as a side effect of VAD. The
remaining problem is *labelling* ~500 existing segments as one of two speakers —
a two-class assignment with strong priors (interviewer turns are short and
question-shaped) — rather than *finding* the boundaries. That is a substantially
easier problem than the one that was given up on.

The likely reason it works now: before the channel-count fix, VAD was operating on
interleaved stereo misread as mono. There were no real silences to find in that
signal, so segmentation could not track turns and diarization built on top of it
had nothing to stand on.

Not yet verified: whether this holds on a second interview, and whether the 6
remaining bleeds cluster on fast back-and-forth exchanges.

#### The generational-loss prediction was wrong, and now clearly so

Phase 0 predicted the original would beat the re-encode, since the re-encode
carries an extra AAC generation and measures further from the WAV (0.9941 vs
0.9978). It lost decisively: 37 substitutions against 63, and the best WER.

So **cross-correlation fidelity does not predict transcription quality.** The
re-encode is measurably further from the reference waveform and measurably better
to transcribe. Whatever the transcode did — filtering, requantisation, a different
resampler — it suited the acoustic model better than the master did.

That makes transcoding a legitimate preprocessing candidate for the corpus, which
is the opposite of the Phase 0 recommendation. It needs confirmation on a second
interview before it becomes policy.

---

## Notes per run — round 1

Harness version at time of writing: **v4** (unchanged by round 1). One paragraph
per experiment: what was predicted, what would have disconfirmed it, what
happened. Block labels are the ones from
[`transcription-tuning-round1-plan.md`](./transcription-tuning-round1-plan.md);
`whisper-runner` should backfill the run ids it minted where they are not already
named here. Every run below is on `108_funky.mp4`, parent `000-original`, unless
stated otherwise.

### R0 (`003-r0`) — verbatim repeat, no axis changed

**Hypothesis:** GPU float jitter would show up somewhere, and `entropy_thold: 3.0`
was believed to sit inside the operating distribution of ordinary prose — a hard
discontinuity parked exactly where last-bit differences live, and therefore an
amplifier that would turn invisible float noise into visibly different text for
whole windows. **Falsifier:** byte-identical `result.json` — which would mean
there is no config-level noise floor at all and no run ever needs repeating.
**What happened:** the falsifier fired, completely. All 417 segments and all 4 936
words were byte-identical; 2 of 4 936 words differed only in DTW timing, one by a
single 20 ms boundary. Every score was identical. This is the most load-bearing
negative in the round: it converts every later single-run delta from "suggestive"
to "real", and it retires the ledger's own noise-floor caveat down to a
*cross-fixture* caveat only. The caveat that survives is not about measurement but
about attribution — the baseline's 63 substitutions are a long tail of singletons,
so a config change that perturbs one boundary can chaotically re-roll a dozen
unrelated tokens. Repeats are useless against that; monotone dose-response across
three or more rungs is the only defence, which is why the plan was built from
ladders and why round 2 keeps that shape.

### B1 — `temperature_inc` → 0.0

**Hypothesis:** the entropy gate at 3.0 was believed to fire spuriously on
ordinary prose, dumping good windows into temperature fallback and producing the
false-start fragments the analyst had found by eye in `000-wav`. Collapsing
`temperatures` to the single rung `[0.0]` should therefore recover most of that
loss without touching `entropy_thold`: `hyp_tokens` down 40–100, `subs` down 4–10,
`ins_content` down 20–40, wall clock down 5–15 %. **Falsifier (reversal branch):**
`ins_content` up by 20 or more, meaning the fallback ladder was load-bearing and
real repetition loops were now being emitted verbatim. **What happened:** the
reversal falsifier fired at a scale nobody had budgeted for. WER 1.8697, `subs`
400, `ins_content` 6 569, `hyp_tokens` 12 749 against a baseline 4 921 — the
output is textbook repetition loop ("realized that that was realized that that
was…"). `anchor_confidence` fell to 14, so the numbers on this row are
directionally certain and numerically loose, but the direction is not in doubt.
This run is what inverted the round's whole reading: the fallback ladder is not
noise, it is the only thing standing between this corpus and runaway repetition,
and the false starts in `000-wav` were repetition the gate **failed to catch**
rather than damage the gate caused. It also, incidentally, cost nothing to
interpret — a config predicted mildly good that comes back catastrophically bad is
worth more than a config predicted good that comes back mildly good.

### Block A — `entropy_thold` ladder (2.4 / 3.0 / 3.2 / 3.4 / 3.45 / 3.46)

**Hypothesis (A1, `entropy_thold` → 2.4, whisper.cpp stock):** 3.0 is
out-of-distribution-high for ordinary prose whose last-32-token entropy computes
around 3.03, so it fires constantly; dropping to stock 2.4 should silence it and
cut `subs` by 5–12 with `hyp_tokens` and `ins_content` falling alongside.
**Falsifier:** `subs` within ±4 of 63 *and* `hyp_tokens` within ±30 of 4 921,
which would mean the gate is not materially firing and block A is dead.
**Hypothesis (A4, `entropy_thold` → 3.4):** included only as a *positive control*
for the block, predicted markedly worse — near the `ln 32 = 3.4657` ceiling
virtually every window should fail the gate and be re-decoded at temperature ≥ 0.2,
so `hyp_tokens` up 150+, `ins_content` up 60+, `subs` up 15+, wall clock up 30–60 %.
Its falsifier was "A4 is *not* markedly worse", which would disprove the mechanism
outright and force every other rung to be re-read as chaotic re-roll.

**What happened:** the direction of the whole axis was backwards, and A4 — the run
included to be bad — is the best configuration the campaign has found. 2.4 was
clearly *worse* than baseline (`subs` 111 vs 63, WER 0.1251 vs 0.0996), and the
ladder improves monotonically upward: 111 → 63 → 44 → 40 → 37 → 37 across
2.4/3.0/3.2/3.4/3.45/3.46, with WER 0.1251 → 0.0996 → 0.0900 → 0.0617 → 0.0621 →
0.0621. Monotone across six points is not a re-roll. 3.45 and 3.46 are
byte-identical, so the axis saturates just below its own `ln 32` ceiling exactly
as the arithmetic predicts. Speaker-boundary lift *rose* as a side effect
(8.96× → 9.94×) and `anchor_confidence` rose 41 → 44, so this is not a case of
better scores bought with worse alignment.

Reading the source afterwards explains why the prediction inverted. There are two
distinct mechanisms, not one. The gate at `whisper.cpp:7517-7536` runs inside the
loop over beam *candidates*: a candidate below threshold is marked `failed` and
`continue`d, so it never enters the `best_score` comparison. That is
**per-candidate pruning**, and a higher threshold prunes harder — which is the
monotone gain. The window-level retry at `7548-7560` is separate and only fires
when the *selected best* candidate is itself failed. The original hypothesis
modelled only the second mechanism and missed the first, which is precisely the
mechanism that dominates. A single-probe design would have tested 2.4, seen
degradation, and closed the axis as a dead end.

The result validates on both held-back interviews at 3.4: substitutions −37 %
(108, 63→40), −52 % (026, 121→58), −28 % (074, 32→23). It is the first change in
the campaign that generalises beyond its tuning fixture. Two caveats stay on the
record. 026's `anchor_confidence` fell 17 → 10 while its scores improved, which is
the borderline of usability on a fixture already scored on partial coverage; the
effect size dwarfs the alignment wobble but that row is the least trustworthy in
the table. And 074 got *terser*, not merely better — deletions rose 29 → 38 while
content insertions collapsed 315 → 60, so most of its headline −62 % WER is
insertions it stopped making, and on this corpus insertions largely track the
transcriber's editing. Lexicon recall is non-monotone across the ladder
(0.892 → 0.952 → 0.928 → 0.940) at 83 term occurrences and must not be read as an
ordering. **Recommended default: `entropy_thold: 3.4`** — preferred over 3.45
because it is the value validated on three interviews rather than one, and because
it is not perched within 0.02 of the ceiling.

### C1 — `no_context` → false

**Hypothesis:** `prompt_past` carries decoded tokens into the next window's
prompt, so recurring proper nouns get primed after first sight; `lexicon.recall`
up 3–6 points, `subs` flat to down 5, with context-drift as the known risk (hard
reversal if `hyp_tokens` rose by more than 100). **Falsifier:** recall moving less
than 5 points *and* `subs` falling less than 8. **What happened:** byte-identical
to the baseline. The config was verified as applied, and the round-1 write-up
recorded it as a genuine null with no explanation. **The explanation was found
while designing round 2 and is now recorded below — `no_context` is a no-op in
this pipeline, and the flag never did what its name says.** See "What round 2's
source reading changed".

### G1 — `beam_size` → 2

**Hypothesis:** a cheap null-closes-the-block probe. If the decoder is
search-limited, halving the beam costs 5–15 substitutions; if not, `subs` barely
moves and the whole decode-search region is low-yield. **Falsifier:** `subs`
within ±5 of 63, which would deprioritise search-widening for the rest of the
campaign and skip beam 8. **What happened:** the decoder *is* search-limited, but
not in the shape the metric summary suggests. WER rose to 0.1325 and `subs`
actually *fell* to 51 — while deletions nearly doubled (56 vs 31) and insertions
rose to 421. A narrower beam produced a transcript that is simultaneously
droppier and more verbose, which is the signature of worse candidates being
selected rather than of a cleaner search. Read as a whole it is clearly worse, and
it is a good illustration of why `subs` alone is not sufficient when `del` moves
that far. Beam 8 was left unrun, and round 2 picks it up — at the new parent,
because the entropy gate now prunes the candidate pool that `beam_size` sizes.

### F1 — `max_len` → 80

**Hypothesis:** `whisper_wrap_segment` is post-processing over an already-decoded
token list, so this axis can move `structure.*` and *nothing else*: segments up
from 417 toward 550–650, mean segment length down toward ~9, `speaker.covered` up
but `speaker.lift` down, and every text metric exactly unchanged. **Falsifier:**
any movement at all in `subs` or `del` — which would mean either that
`whisper_wrap_segment` is not text-preserving or that the harness's
segment-to-token assembly is segmentation-sensitive, and *either* would invalidate
every cross-config text comparison where segment counts differ. **What happened:**
segments 417 → 554 and lift 8.96× → 6.75×, with text essentially unchanged
(`subs` 67 vs 63 — a small re-roll consistent with wrap-induced boundary effects
in the harness's alignment, not a systematic shift). The prediction held, so this
run doubles as a clean bill of health for the instrument: segmentation-driven
comparisons elsewhere in the ledger are not confounded by the scorer. It also
locates the caption-length trade-off — finer segments raise raw boundary coverage
while *lowering* lift, exactly the gaming direction the ledger warns about.

### Blocks D / E / I / J / K — all seven VAD runs, null by construction

**Hypothesis (across the block):** Silero's output is destructive rather than
advisory, so `speech_pad_ms`, `min_silence_duration_ms`, `threshold`,
`max_speech_duration_s` and `samples_overlap_s` gate what content can be
transcribed at all; `speech_pad_ms` in particular was predicted to fix the
onset-clipping signature visible in the lexicon confusions (`letra` for
`Letraset`, `apisala` for `Episalla`). **Falsifier (per rung):** deletions and
substitutions both flat, which would close the rung. **What happened:** all seven
returned metrics byte-identical to the parent — and a range that wide producing
*exactly zero* change is a symptom, not a null. `WhisperState::full()` calls
`whisper_full_with_state`, and the `params.vad` check lives only in `whisper_full`
(`whisper.cpp:7743`) and `whisper_full_parallel` (7766). `whisper_full_with_state`
never reads it. So `enable_vad(true)`, the silero model path and
`max_speech_duration_s: 60.0` have been inert in **every run ever made**,
baselines included. Two earlier ledger claims are wrong as a consequence and are
corrected in the round-1 section above: the ~9× speaker-boundary lift is Whisper's
own timestamp-token segmentation and not silero, which makes the diarization
prospect *better* rather than worse; and the speculation that segmentation
improved because VAD had been operating on interleaved stereo is unfounded (the
channel-count bug and its fix were both real, but VAD played no part). Every VAD
axis is unavailable until someone wires up whisper-rs's separate
`WhisperVadContext` and feeds it filtered samples — a design decision, since the
pipeline demonstrably works without it. **Do not propose VAD axes until that
lands.** Seven runs, roughly 30 minutes of GPU, bought one architectural fact; the
cheaper purchase would have been reading `whisper_state/mod.rs:299` first, and
that is the lesson to carry into round 2.

---

## What round 2's source reading changed

Recorded here because two of these findings close axes without a run and one of
them explains a round-1 null the ledger had left open. All are checkable in
`whisper.cpp` and `whisper-rs-0.16.0`; none cost GPU time.

### `no_context` is a no-op in this pipeline — the C1 null is explained

`no_context` is read exactly once per `whisper_full_with_state` call, at
`whisper.cpp:6900`, where it clears `prompt_past0`/`prompt_past1`. It is **not**
consulted per window. The per-window refill at `7590-7601` has no `no_context`
guard at all:

```c
prompt_past1.clear();
if (!params.carry_initial_prompt && !prompt.empty() && prompt.front() == whisper_token_prev(ctx)) {
    prompt_past1.insert(prompt_past1.end(), prompt.begin() + 1, prompt.end() - prompt_init.size());
}
if (!is_no_speech) {
    for (int i = 0; i < result_len; ++i) prompt_past1.push_back(tokens_cur[i].id);
}
```

`pipeline.rs` calls `whisper::transcribe` once per file and `transcribe` calls
`ctx.create_state()` fresh each time, so both buffers are already empty when 6900
runs. Clearing them is a no-op, and the rolling context then accumulates across
windows **regardless of the flag**. C1 was therefore byte-identical for a
structural reason, not a modelling one.

Two consequences. First, **the campaign has been running with rolling context
switched on the whole time while believing it was off** — every baseline, every
round-1 run. Second, `no_context` must not be proposed again in either direction;
it cannot move anything.

The real context switch is `n_max_text_ctx`, which guards the prompt-assembly
block at `7090` and the budget at `7094`. It is exposed by whisper-rs
(`set_n_max_text_ctx`) but is not a `TranscribeConfig` field, so it needs a
three-line addition before it can be run.

### `initial_prompt` decays after ~223 tokens — it is near-inert as currently wired

`set_initial_prompt` does not set `carry_initial_prompt`, so the prompt takes the
`else` branch at `6937-6942` and is pushed into `prompt_past1` — the *rolling*
buffer, not the static one. The take at `7106` keeps only the last
`max_prompt_ctx - 1` tokens, and `max_prompt_ctx = min(n_max_text_ctx,
n_text_ctx/2)` = **223** for large-v3. So the prompt sits at the front of an
ever-growing buffer and is truncated away once ~223 tokens of transcript have
accumulated — roughly two 30 s windows, about **60 seconds of a 34-minute
interview**.

The good news is that the prompt *is* live under our current `no_context: true`,
because 6900 runs before 6924 — the clear cannot remove a prompt that has not been
installed yet. The bad news is that its reach is ~3 % of the file, so any effect
on `subs` over 4 589 tokens would be 1–3 substitutions: inside the
unattributable-re-roll band the R0 note describes. **The domain-prompt axis is
therefore not worth a GPU run in its current wiring**, despite being nominally the
campaign's highest-leverage knob. It becomes worth running the moment
`carry_initial_prompt` is set, which pins the prompt into `prompt_past0` and
re-prepends it to *every* window (`7098-7103`). whisper-rs 0.16 does not expose
it and `FullParams::fp` is `pub(crate)`, so this needs a vendored patch or an
upstream PR — a small one, and the highest-value non-GPU task on the board.

### The fallback counters are not cheaply observable — retract the round-1 ask

The round-1 plan asked for `fallbacks = %d p / %d h` (`whisper.cpp:4271`) in
`run.log`. It is not reachable. `whisper_print_timings` reads `ctx->state`, but
this pipeline decodes through a caller-created `WhisperState`, so that pointer is
null and the line is skipped. `n_fail_p`/`n_fail_h` have no public getter —
`whisper_get_timings` returns only the five `_ms` fields (`whisper.h:438-445`) —
and the `WHISPER_LOG_DEBUG` lines that report each entropy prune and each
temperature retry compile to nothing unless `WHISPER_DEBUG` is defined at build
time (`whisper.cpp:128-132`). **Stop asking for this metric.** The reachable
proxies are *wall clock* (retry count is proportional to extra decode passes) and
`WhisperSegment::no_speech_probability()`, which whisper-rs does expose and which
would let the `no_speech_thold` question be screened from an existing run's
artefacts rather than probed with a new one.

### Two structural facts that shape round 2's decode block

- **`beam_size` only applies at temperature 0.** At `7046-7056`, a beam-search
  run uses `beam_size` decoders when `t_cur == 0` and `params.greedy.best_of`
  otherwise — which is `-1` under this strategy, clamped to 1. Every fallback rung
  is therefore a *single* temperature-sampled decoder, not a beam. So `beam_size`
  sizes exactly the pool the entropy gate prunes from, and nothing else.
- **History conditioning is dropped above t = 0.5**
  (`WHISPER_HISTORY_CONDITIONING_TEMP_CUTOFF`, `whisper.cpp:145`, used at `7090`).
  Rungs at 0.5 and above decode context-free, so `temperature_inc` changes not
  only how many retries are available but how many of them see prior context.

### Axes closed without a run, and why

| Axis | Closed because |
|---|---|
| `no_context` (either direction) | Structural no-op: cleared once on an already-empty buffer, refilled per window with no guard (`whisper.cpp:6900` vs `7590-7601`). |
| `initial_prompt` (as wired) | Reaches only the first ~223 decoded tokens ≈ 60 s of a 34 min file. Predicted effect is inside the unattributable band. Re-open when `carry_initial_prompt` is exposed. |
| `temperature` (start of ladder) | Any value above 0 moves rung 0 off `t_cur == 0`, which switches the first and best pass from beam search to single-decoder sampling (`7046-7056`). Strictly worse by construction. |
| `suppress_nst` | Every token it suppresses (`whisper.cpp:6095-6100`) is punctuation or a music glyph, plus leading `" -"` / `" '"`. The harness normalises punctuation away, so it can only produce second-order re-rolls — movement would be real but unattributable. Not an experiment. |
| `patience` | Dead knob, closed in round 1 by inspection. |
| `max_len` / `split_on_word` | Round 1 proved text-invariance. The remaining question is caption usability, which carries no scored metric and can be settled by eye from existing artefacts. |
| All VAD axes | Inert until `WhisperVadContext` is wired up. See round 1. |
| `audio.*` | Closed by Phase 0. |
