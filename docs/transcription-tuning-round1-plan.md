# Round 1 tuning plan

Produced by the `whisper-tuner` agent from the ledger and the vendored whisper.cpp source.

I have what I need. Here is the round-1 design.

---

# Round 1 — Whisper tuning campaign on `108_funky.mp4`

## 0. What I found reading the source before designing anything

Three things in `/home/ubuntu/auohp/packages/core/src/transcription/config.rs` and the vendored `whisper.cpp` change what round 1 should be. All are checkable in source, none cost a run.

**(a) `entropy_thold: 3.0` is not the default, and it is set to an extreme value.** whisper.cpp's default is `2.4` (`whisper.cpp:5954`). The gate is:

```c
// whisper.cpp:7527
if (decoder.sequence.result_len > 32 && decoder.sequence.entropy < params.entropy_thold) {
    decoder.failed = true;   // -> temperature fallback, window re-decoded
```

`sequence.entropy` is the Shannon entropy of the **token multiset of the last 32 tokens** (`whisper.cpp:6584-6604`). Its ceiling is `ln(32) = 3.466`. So `3.0` demands that 32 consecutive tokens be almost all distinct. Ordinary English prose containing four `the`, four commas and two `of` in 32 tokens computes to ≈3.03 — a hair above the gate. **This threshold sits inside the distribution of normal prose, not out in the repetition-loop tail.** Every time it fires, the window is discarded and re-decoded at t=0.2, 0.4, … which is precisely the mechanism that produces the "false start, restart" fragments the analyst identified in `000-wav`.

**(b) The temperature-fallback ladder is live and nobody knows it.** `temperature_inc` is `None` → whisper.cpp default `0.2` → `temperatures = [0.0, 0.2, … 1.0]` (`whisper.cpp:6854`). So the pipeline is *not* "beam search at temperature 0". It is beam-at-0 with a five-rung stochastic fallback ladder that has been made **more** likely to fire than stock.

**(c) `decode.patience` is a dead knob.** `patience` appears exactly three times in `whisper.cpp` — the struct field and two default initialisers. It is never read by the decode loop in this version. Setting it to `1.0` (we do) versus `-1.0` (stock) has literally no effect. **Do not spend a run on it.** Consider that axis closed by inspection.

**(d) `max_len` is silently a no-op unless `token_timestamps` is true** (`whisper.cpp:7649-7654`), and `split_on_word` is silently a no-op unless `max_len > 0` (`whisper.cpp:6059`). Both preconditions hold for us, but it means **`split_on_word: true` alone is a provable no-op and must never be proposed as a standalone run.** It also means `whisper_wrap_segment` is pure post-processing of an already-decoded token list — it cannot change a single token, so `max_len`/`split_on_word` are predicted to move `structure.*` and nothing else.

**(e) VAD is destructive, not advisory.** `whisper.cpp:6641-6690` builds a *new* sample buffer containing only Silero's speech segments, glued with 0.1 s of silence, and decodes that. Everything Silero rejects is destroyed before Whisper sees it. So `threshold`, `speech_pad_ms` and `min_silence_duration_ms` gate what content can possibly be transcribed. Related: `whisper.cpp:5383` merges two speech runs whenever the silence between them is shorter than `2 × speech_pad_samples` — so raising `speech_pad_ms` *deletes* segment boundaries, which is exactly what generates the ledger's 8.96× speaker-change lift.

---

## 1. Your determinism question

**The algorithm is deterministic; the config we are running is not obviously so.**

whisper.cpp seeds every decoder explicitly at the top of each `whisper_full_with_state`:

```c
// whisper.cpp:6894
decoder.rng = std::mt19937(j);   // j = decoder index
```

There is no time-, PID- or entropy-derived seed anywhere in the file. So even the fallback temperatures sample from a fixed stream. Silero VAD is a plain forward pass. On CPU this pipeline is deterministic by construction, and your premise holds.

The open question is GPU float reproducibility, and finding (a) is why it is not academic. If two CUDA runs differ in the last bits of a logit, that difference is normally invisible — argmax and beam ranking absorb it. But `entropy_thold: 3.0` places a **hard discontinuity right in the middle of the operating distribution**: a sequence computing 3.0001 is accepted, one computing 2.9999 is thrown away and re-decoded at a different temperature, producing wholly different text for that window. That is an amplifier sitting exactly where float jitter lives. So the config most likely to be non-reproducible is the one we are using as the parent.

### The cheap test — run it first, before anything else

**R0**: re-run the `000-original` config verbatim under a new run id on `108_funky.mp4`, then `diff` `result.json` against `000-original/result.json`. One run, ~260 s, 4 % of budget. Three possible outcomes:

- **Byte-identical.** Determinism holds. Every subsequent run buys clean signal, the ledger's noise-floor caveat collapses to a cross-fixture caveat only, and no config ever needs repeating.
- **Differs.** The `score.json` delta between R0 and `000-original` *is* the config-level noise floor — the number the ledger currently lacks and has been approximating with a cross-fixture argument.
- **Differs, and the git SHA differs from `000-original`'s manifest.** Then R0 is confounded with code drift and must be re-read as a re-baseline, not a determinism test. The runner must record `git rev-parse HEAD`; `git status --porcelain` currently shows only an untracked `cat`, so the tree is otherwise clean.

R0 doubles as a re-baseline of the parent at the current SHA, which round 1 needs anyway.

### If determinism does NOT hold

1. The R0 delta becomes the reporting threshold. Any single-run delta smaller than it is uninterpretable, full stop.
2. The sweeps below **do not change shape** — they are already 3–4 rung dose-response ladders rather than single probes, which is the correct design under noise. I would read only the monotone trend across a ladder, never one rung against the parent.
3. I would cut the two lowest-yield ladders (`samples_overlap_s`, `max_speech_duration_s`) to buy two repeats of the round-1 winner.
4. I would **promote B1 (`temperature_inc: 0.0`) to first position**, because it removes the discontinuity. A config that is reproducible *and* simpler to reason about is a finding worth having on its own, independent of whether it scores better.

### The caveat that survives either outcome, and it is the important one

**Determinism eliminates measurement noise. It does not eliminate sensitivity.** The baseline's 63 substitutions are a long tail of singletons and pairs — the modal count in `taxonomy.substitutions` is 1, the maximum is 2. That is a sum of many near-independent events, so a config change that shifts one VAD boundary by 10 ms can chaotically re-roll a dozen unrelated tokens. A deterministic 5-substitution delta is a *real* difference between those two configs, but it is not necessarily an *attributable effect of the axis*.

The defence against that is not repeats — repeats are useless against it. The defence is **monotone dose-response across three or more levels of the same axis**, which is why every block below is a ladder. This is the single most important design consequence of your question, and it is true whether or not R0 comes back byte-identical.

---

## 2. On tuning fixture 108 — I agree, with two caveats

I agree with `108_funky.mp4`: production inputs are masters, 108 has full coverage (`partial_coverage: false`), 4589 truth tokens, and `anchor_confidence: 41`. 026 would force every run to be scored on a partial-coverage overlap, which is a worse instrument for a tuning campaign. Two things to hold onto:

1. **We are tuning on the worst of the three 108 inputs by the primary metric.** The ledger's strongest replicated finding is that the re-encode has the fewest substitutions (37 vs 63 on 108, 55 vs 121 on 026). That is fine — masters are what production feeds — but it means some round-1 winners may be *compensating for the master's acoustics* rather than improving decoding. So validation should include `108_truth.mp4`, not only 026/074. Otherwise you cannot separate "better config" from "better config for masters".
2. **Only `subs` is well-powered on this fixture.** Lexicon recall has 76 term occurrences and `speaker.changes` is 63. Per the ledger's own revised guidance, recall moves under ~8 points are inside its sampling noise. I rank on `subs` and `hyp_tokens`, and treat `lexicon.recall` and `speaker.lift` as directional only.

---

## 3. Instrument notes for the runner and `eval-harness` (no GPU cost)

- **`run.log` is blind where round 1 needs sight.** whisper.cpp logs `fallbacks = %3d p / %3d h` at INFO (`whisper.cpp:4271`, via `whisper_print_timings`) and `detected %d speech segments` at INFO (`whisper.cpp:6650`). Neither appears in `/mnt/s3/fs1/out/runs/000-original/run.log`, which stops after `Whisper: 417 segments`. Blocks A, B and the VAD blocks would be far better instrumented with those two lines. That is a runner/harness observability fix, not a tuning axis, and I am not proposing it as an experiment — but if it lands cheaply before round 1 starts, blocks A and B become directly readable instead of inferential.
- **`ins_content` is not a fragment detector on this fixture.** The baseline's top insertions are `and`(34), `i`(29), `you`(27), `know`(21), `um`(12), `okay`(6) — faithful disfluency capture that the human transcriber removed, not hallucination. Use **`wer.hyp_tokens`** (4921 against 4589 truth) as the primary verbosity/fragment scalar: it is a single number over ~4900 tokens and is well powered. Read `ins_content` alongside it, never alone.
- **The filler classifier is leaking.** `ins_filler` is 91, but `you`(27) + `know`(21) + `um`(12) alone is 60, and `you know` is in `FILLER_PHRASES`. The phrase is being split across alignment ops and scored as content. Worth a look by `eval-harness`; it inflates `ins_content` uniformly so it does not bias comparisons, but it does make the absolute number misleading.
- **`suppress_nst` is deliberately not proposed.** There is no evidence of non-speech tokens anywhere in the baseline taxonomy. Proposing it would be padding the list.
- **`token_timestamps` is not tunable.** DTW word timing is a product requirement for the caption editor, and turning it off would silently disable `max_len` too (finding (d)).

---

## 4. Config format constraint — this will bite the runner

`TranscribeConfig` carries `#[serde(default)]` at the **struct level only**. `AudioConfig`, `DecodeConfig` and `VadConfig` do not. Serde does not treat `Option<T>` as optional without `#[serde(default)]`. Therefore **any `config.json` that supplies a `decode` or `vad` object must supply every field of it**, `null`s included. A partial object fails to parse. Every JSON below is complete and ready to write verbatim.

The parent config, `P0` (= `000-original` effective):

```json
{
  "audio": {"resample_chunk":4096,"sinc_len":256,"f_cutoff":0.95,"oversampling_factor":256,"interpolation":"linear"},
  "decode": {"language":"en","beam_size":5,"patience":1.0,"entropy_thold":3.0,"logprob_thold":null,"no_speech_thold":null,"temperature":null,"temperature_inc":null,"no_context":true,"suppress_nst":null,"initial_prompt":null,"max_len":null,"split_on_word":null,"token_timestamps":true},
  "vad": {"enabled":true,"threshold":null,"min_speech_duration_ms":null,"min_silence_duration_ms":null,"max_speech_duration_s":60.0,"speech_pad_ms":null,"samples_overlap_s":null}
}
```

---

## 5. Execution order

Ordered so that a truncated budget still buys the most. Blocks are contiguous where possible so ladders stay readable.

| # | Run | Block | Axis → value | Parent | Cond. |
|---|---|---|---|---|---|
| 1 | R0 | — | *(none — verbatim repeat)* | 000-original | — |
| 2 | B1 | B | `temperature_inc` → 0.0 | 000-original | — |
| 3 | A1 | A | `entropy_thold` → 2.4 | 000-original | — |
| 4 | A4 | A | `entropy_thold` → 3.4 | 000-original | yes |
| 5 | C1 | C | `no_context` → false | 000-original | — |
| 6 | D1 | D | `speech_pad_ms` → 100 | 000-original | — |
| 7 | G1 | G | `beam_size` → 2 | 000-original | — |
| 8 | A2 | A | `entropy_thold` → 2.0 | 000-original | yes |
| 9 | D2 | D | `speech_pad_ms` → 250 | 000-original | — |
| 10 | E1 | E | `min_silence_duration_ms` → 300 | 000-original | — |
| 11 | F1 | F | `max_len` → 80 | 000-original | — |
| 12 | K1 | K | `vad.threshold` → 0.35 | 000-original | — |
| 13 | E2 | E | `min_silence_duration_ms` → 700 | 000-original | — |
| 14 | A3 | A | `entropy_thold` → 1.5 | 000-original | yes |
| 15 | J1 | J | `max_speech_duration_s` → 120.0 | 000-original | — |
| 16 | K2 | K | `vad.threshold` → 0.65 | 000-original | — |
| 17 | I1 | I | `samples_overlap_s` → 0.0 | 000-original | — |
| 18 | D3 | D | `speech_pad_ms` → 400 | 000-original | yes |
| 19 | F2 | F | `split_on_word` → true | **F1** | yes |
| 20 | G2 | G | `beam_size` → 8 | 000-original | yes |
| 21 | J2 | J | `max_speech_duration_s` → 30.0 | 000-original | yes |
| 22 | F3 | F | `max_len` → 45 | **F2** | yes |
| 23 | B2 | B | `temperature_inc` → 0.4 | 000-original | yes |
| 24 | H1 | H | `no_speech_thold` → 0.3 | 000-original | yes |

24 runs ≈ 105 min of GPU under the lock, plus CPU scoring. Nine are conditional; dropping all of them lands at 15 runs ≈ 65 min.

---

## 6. The experiments

### Block A — `entropy_thold`. The strongest lead in the whole space.

Why this axis and not another: it is the only *non-default, out-of-distribution* value in the shipped config, its mechanism is a hard discontinuity in the middle of normal prose, and nobody has ever touched it.

---

**A1 — `entropy_thold` → 2.4 (whisper.cpp stock)**

```json
{
  "audio": {"resample_chunk":4096,"sinc_len":256,"f_cutoff":0.95,"oversampling_factor":256,"interpolation":"linear"},
  "decode": {"language":"en","beam_size":5,"patience":1.0,"entropy_thold":2.4,"logprob_thold":null,"no_speech_thold":null,"temperature":null,"temperature_inc":null,"no_context":true,"suppress_nst":null,"initial_prompt":null,"max_len":null,"split_on_word":null,"token_timestamps":true},
  "vad": {"enabled":true,"threshold":null,"min_speech_duration_ms":null,"min_silence_duration_ms":null,"max_speech_duration_s":60.0,"speech_pad_ms":null,"samples_overlap_s":null}
}
```

- **Parent:** `000-original`
- **Hypothesis:** the repetition gate stops firing on ordinary prose, so fewer windows get thrown into t=0.2+ fallback. `hyp_tokens` drops from 4921 toward 4820–4880; `ins_content` drops from 272 toward 230–255; `subs` drops from 63 by 5–12. `dels` flat (fallback does not delete audio). `wer.rate` down.
- **Falsifier:** `subs` within ±4 of 63 **and** `hyp_tokens` within ±30 of 4921 → the gate is not materially firing at 3.0 on this fixture and block A is a dead end. Record the negative and stop the ladder.

---

**A4 — `entropy_thold` → 3.4 (positive control)** · *conditional*

```json
{
  "audio": {"resample_chunk":4096,"sinc_len":256,"f_cutoff":0.95,"oversampling_factor":256,"interpolation":"linear"},
  "decode": {"language":"en","beam_size":5,"patience":1.0,"entropy_thold":3.4,"logprob_thold":null,"no_speech_thold":null,"temperature":null,"temperature_inc":null,"no_context":true,"suppress_nst":null,"initial_prompt":null,"max_len":null,"split_on_word":null,"token_timestamps":true},
  "vad": {"enabled":true,"threshold":null,"min_speech_duration_ms":null,"min_silence_duration_ms":null,"max_speech_duration_s":60.0,"speech_pad_ms":null,"samples_overlap_s":null}
}
```

- **Parent:** `000-original`
- **Condition:** run only if A1 moved anything. Skip if A1 was null.
- **Hypothesis:** 3.4 is just under the `ln(32) = 3.466` ceiling, so virtually **every** window fails the gate and every window gets re-decoded at temperature ≥ 0.2. This should be **dramatically worse**: `hyp_tokens` up 150+, `ins_content` up 60+, `subs` up 15+, and visible fragment restarts in the transcript. Wall clock up 30–60 % from the extra decode passes.
- **Falsifier:** if A4 is *not* markedly worse — `ins_content` up by less than 40 and `hyp_tokens` up by less than 80 — then the gate is not firing even at 3.4, the mechanism is disproved, and **any movement observed in A1/A2/A3 must be re-read as chaotic re-roll rather than an entropy effect.**
- **Why spend a run on a config predicted to be bad:** we cannot see `fallbacks = N h` in `run.log`. A4 is the cheapest available proof that the gate fires at all, and a large predicted-bad result is stronger evidence than a small predicted-good one. It is the positive control for the entire block.

---

**A2 — `entropy_thold` → 2.0** · *conditional on A1 showing movement*

Same as A1 with `"entropy_thold":2.0`.

```json
{
  "audio": {"resample_chunk":4096,"sinc_len":256,"f_cutoff":0.95,"oversampling_factor":256,"interpolation":"linear"},
  "decode": {"language":"en","beam_size":5,"patience":1.0,"entropy_thold":2.0,"logprob_thold":null,"no_speech_thold":null,"temperature":null,"temperature_inc":null,"no_context":true,"suppress_nst":null,"initial_prompt":null,"max_len":null,"split_on_word":null,"token_timestamps":true},
  "vad": {"enabled":true,"threshold":null,"min_speech_duration_ms":null,"min_silence_duration_ms":null,"max_speech_duration_s":60.0,"speech_pad_ms":null,"samples_overlap_s":null}
}
```

- **Parent:** `000-original`
- **Hypothesis:** continues A1's direction but with diminishing return, because by 2.4 the gate is already mostly silent on prose. Expect `subs` and `hyp_tokens` between A1 and A3, closer to A1. This rung's job is to establish *monotonicity*, which is what distinguishes a real axis effect from a chaotic re-roll.
- **Falsifier:** A2 lands *outside* the interval bracketed by A1 and A3 by more than the R0 noise floor (or more than ±6 subs if determinism holds) → the ladder is not monotone, the axis is re-rolling rather than acting, and no causal claim can be made from block A regardless of which rung scored best.

---

**A3 — `entropy_thold` → 1.5 (gate effectively off)** · *conditional on A1/A2 monotone*

Same as A1 with `"entropy_thold":1.5`.

```json
{
  "audio": {"resample_chunk":4096,"sinc_len":256,"f_cutoff":0.95,"oversampling_factor":256,"interpolation":"linear"},
  "decode": {"language":"en","beam_size":5,"patience":1.0,"entropy_thold":1.5,"logprob_thold":null,"no_speech_thold":null,"temperature":null,"temperature_inc":null,"no_context":true,"suppress_nst":null,"initial_prompt":null,"max_len":null,"split_on_word":null,"token_timestamps":true},
  "vad": {"enabled":true,"threshold":null,"min_speech_duration_ms":null,"min_silence_duration_ms":null,"max_speech_duration_s":60.0,"speech_pad_ms":null,"samples_overlap_s":null}
}
```

- **Parent:** `000-original`
- **Hypothesis:** the asymptote. Entropy 1.5 corresponds to roughly 4–5 effective distinct tokens in 32, i.e. only a genuine repetition loop trips it. Scores should be at or barely past A2, and should be near-identical to B1 (which disables the ladder outright by a different mechanism).
- **Falsifier:** A3 differs from B1 by more than the R0 noise floor on `subs` **or** by more than 40 on `hyp_tokens`. Those two configs should converge — both leave beam-at-t0 output untouched for essentially every window. If they diverge, one of my two readings of the fallback path is wrong and block A's conclusion is not safe to act on. **This cross-check between two independently-derived configs is the most valuable thing in the block.**

---

### Block B — `temperature_inc`. The on/off switch for the whole fallback mechanism.

Why this axis and not another: it tests the same mechanism as block A with a single binary, orthogonal knob. If B1 comes back byte-identical to R0, blocks A **and** B are both closed in two runs and roughly a fifth of the budget is freed for the VAD blocks. That is the highest information-per-run in the plan, which is why it runs second.

---

**B1 — `temperature_inc` → 0.0**

```json
{
  "audio": {"resample_chunk":4096,"sinc_len":256,"f_cutoff":0.95,"oversampling_factor":256,"interpolation":"linear"},
  "decode": {"language":"en","beam_size":5,"patience":1.0,"entropy_thold":3.0,"logprob_thold":null,"no_speech_thold":null,"temperature":null,"temperature_inc":0.0,"no_context":true,"suppress_nst":null,"initial_prompt":null,"max_len":null,"split_on_word":null,"token_timestamps":true},
  "vad": {"enabled":true,"threshold":null,"min_speech_duration_ms":null,"min_silence_duration_ms":null,"max_speech_duration_s":60.0,"speech_pad_ms":null,"samples_overlap_s":null}
}
```

- **Parent:** `000-original`
- **Hypothesis:** `temperature_inc <= 0` collapses `temperatures` to `[0.0]` (`whisper.cpp:6858`), so no window is ever re-decoded regardless of the entropy gate. If the gate is firing spuriously, B1 recovers most of what A1–A3 recover without touching `entropy_thold`: `hyp_tokens` down 40–100, `subs` down 4–10, `ins_content` down 20–40. Wall clock down 5–15 %. B1 should also be *strictly* deterministic, since it removes the discontinuity — so B1 is a second, independent probe of the R0 question.
- **Falsifier, two-sided:**
  - *Null:* `result.json` byte-identical to R0's → the gate never fires on this fixture, and **blocks A and B are both closed**. Record it as the round's most valuable negative and reallocate their eight run slots to D/E/K.
  - *Reversal:* `ins_content` up by ≥ 20, or any single token in `taxonomy.insertions` exceeding count 12 (baseline top is `and` at 34, but the top *content* word is far lower) → the fallback ladder was load-bearing, real repetition loops are now being emitted verbatim, and `entropy_thold` should be tuned *up*-ward-cautiously rather than removed. In that case A1–A3 lose priority and B2 gains it.

---

**B2 — `temperature_inc` → 0.4** · *conditional*

```json
{
  "audio": {"resample_chunk":4096,"sinc_len":256,"f_cutoff":0.95,"oversampling_factor":256,"interpolation":"linear"},
  "decode": {"language":"en","beam_size":5,"patience":1.0,"entropy_thold":3.0,"logprob_thold":null,"no_speech_thold":null,"temperature":null,"temperature_inc":0.4,"no_context":true,"suppress_nst":null,"initial_prompt":null,"max_len":null,"split_on_word":null,"token_timestamps":true},
  "vad": {"enabled":true,"threshold":null,"min_speech_duration_ms":null,"min_silence_duration_ms":null,"max_speech_duration_s":60.0,"speech_pad_ms":null,"samples_overlap_s":null}
}
```

- **Parent:** `000-original`
- **Condition:** run **only** if B1 came back materially *worse* than R0 — i.e. the ladder is load-bearing and we need to know its shape rather than remove it.
- **Hypothesis:** a three-rung ladder (0, 0.4, 0.8) instead of six. Fewer rungs means fewer wasted re-decodes but a coarser jump to high temperature. Expect scores between B1 and baseline, and wall clock between them.
- **Falsifier:** B2 lands outside the B1–baseline interval on `subs` by more than the noise floor → `temperature_inc` is not acting as a smooth ladder-length control and the block should be abandoned rather than refined.

---

### Block C — `no_context`. The only remaining lexicon lever, given the prompt is out of scope.

Why this axis and not another: with `set_initial_prompt` off the table, `no_context: false` is the *only* mechanism left that carries vocabulary across decode windows. The nine lexicon misses on the baseline are one-shot proper nouns — `Wojnarowicz`, `Letraset`, `Deb Levine`, `Joy Episalla`, `Working Document` — exactly the class that context priming targets.

---

**C1 — `no_context` → false**

```json
{
  "audio": {"resample_chunk":4096,"sinc_len":256,"f_cutoff":0.95,"oversampling_factor":256,"interpolation":"linear"},
  "decode": {"language":"en","beam_size":5,"patience":1.0,"entropy_thold":3.0,"logprob_thold":null,"no_speech_thold":null,"temperature":null,"temperature_inc":null,"no_context":false,"suppress_nst":null,"initial_prompt":null,"max_len":null,"split_on_word":null,"token_timestamps":true},
  "vad": {"enabled":true,"threshold":null,"min_speech_duration_ms":null,"min_silence_duration_ms":null,"max_speech_duration_s":60.0,"speech_pad_ms":null,"samples_overlap_s":null}
}
```

- **Parent:** `000-original`
- **Hypothesis:** `prompt_past` carries decoded tokens into the next window's prompt (`whisper.cpp:6900`, `7590-7601`). Recurring terms get primed after first sight, so *repeat* occurrences of multi-occurrence terms improve. `lexicon.recall` up 3–6 points (0.882 → 0.91–0.94); `subs` flat to down 5. Known risk: context-induced drift, where the model starts echoing its own prior output.
- **Falsifier:** `lexicon.recall` moves by less than 5 points **and** `subs` falls by less than 8 → context priming is not paying for its risk on this fixture. (Bar set at 5 rather than the ledger's 8 *only if* R0 proves determinism; if R0 shows drift, the bar reverts to 8 points and I would want C1 repeated before acting.) Separate hard reversal: `hyp_tokens` up by more than 100 → context drift is active and `no_context: false` should be rejected outright regardless of recall.
- **Interaction to flag, do not resolve in round 1:** `no_context: false` plus `entropy_thold: 3.0` is a hazardous pair. Context priming raises repetition risk exactly where the aggressive entropy gate then dumps windows into high-temperature fallback. If both A and C produce winners, **their combination needs its own run and must not be assumed additive.** That is a round-2 item.

---

### Block D — `speech_pad_ms`. Onset clipping.

Why this axis and not another: finding (e) — Silero's output is destructive, and `speech_pad_ms` at the default 30 ms is the entire margin protecting word onsets. The baseline's lexicon confusions have the exact signature of clipped onsets: `letra` for `Letraset`, `apisala` for `Episalla`, `leone` for `Dione`.

---

**D1 — `speech_pad_ms` → 100**

```json
{
  "audio": {"resample_chunk":4096,"sinc_len":256,"f_cutoff":0.95,"oversampling_factor":256,"interpolation":"linear"},
  "decode": {"language":"en","beam_size":5,"patience":1.0,"entropy_thold":3.0,"logprob_thold":null,"no_speech_thold":null,"temperature":null,"temperature_inc":null,"no_context":true,"suppress_nst":null,"initial_prompt":null,"max_len":null,"split_on_word":null,"token_timestamps":true},
  "vad": {"enabled":true,"threshold":null,"min_speech_duration_ms":null,"min_silence_duration_ms":null,"max_speech_duration_s":60.0,"speech_pad_ms":100,"samples_overlap_s":null}
}
```

- **Parent:** `000-original`
- **Hypothesis:** 70 ms more audio kept on each side of every speech run. `dels` down from 31 toward 22–28; the truncation-signature substitutions resolve, so `subs` down 3–8 and `lexicon.recall` up as a consequence. Segment count down slightly from 417 (the `2 × speech_pad` merge rule at `whisper.cpp:5383` now absorbs silences under 200 ms).
- **Falsifier:** `dels` within ±4 of 31 **and** `subs` within ±4 of 63 → onset clipping is not a live defect and the ladder stops here. Note `dels` is only 31 to begin with and the deletion taxonomy is single function words (`i`×4, `the`×2, `think`×2) rather than blocks, so a null here is genuinely plausible — the pad axis is a hypothesis about *fidelity at boundaries*, not about swallowed audio.

---

**D2 — `speech_pad_ms` → 250**

Same as D1 with `"speech_pad_ms":250`.

```json
{
  "audio": {"resample_chunk":4096,"sinc_len":256,"f_cutoff":0.95,"oversampling_factor":256,"interpolation":"linear"},
  "decode": {"language":"en","beam_size":5,"patience":1.0,"entropy_thold":3.0,"logprob_thold":null,"no_speech_thold":null,"temperature":null,"temperature_inc":null,"no_context":true,"suppress_nst":null,"initial_prompt":null,"max_len":null,"split_on_word":null,"token_timestamps":true},
  "vad": {"enabled":true,"threshold":null,"min_speech_duration_ms":null,"min_silence_duration_ms":null,"max_speech_duration_s":60.0,"speech_pad_ms":250,"samples_overlap_s":null}
}
```

- **Parent:** `000-original`
- **Hypothesis:** text metrics continue D1's direction with diminishing return, but **the segmentation cost now bites**: at 250 ms pad, any silence under 500 ms stops being a boundary. Turn-taking pauses are precisely what generates the ledger's 8.96× lift. Expect segments down from 417 toward 300–340, `mean_segment_tokens` up from 12.0 toward 15, `bleed` up from 16.
- **Falsifier / stop-rule:** if `speaker.lift` drops below 6.0×, the segmentation cost is unacceptable regardless of the text gain — **skip D3** and record the pad ceiling as lying between 100 and 250 ms. This is the concrete resolution of the granularity-vs-bleed tension the ledger flagged as unsettled at the end of the 026 section.

---

**D3 — `speech_pad_ms` → 400** · *conditional*

Same with `"speech_pad_ms":400`.

```json
{
  "audio": {"resample_chunk":4096,"sinc_len":256,"f_cutoff":0.95,"oversampling_factor":256,"interpolation":"linear"},
  "decode": {"language":"en","beam_size":5,"patience":1.0,"entropy_thold":3.0,"logprob_thold":null,"no_speech_thold":null,"temperature":null,"temperature_inc":null,"no_context":true,"suppress_nst":null,"initial_prompt":null,"max_len":null,"split_on_word":null,"token_timestamps":true},
  "vad": {"enabled":true,"threshold":null,"min_speech_duration_ms":null,"min_silence_duration_ms":null,"max_speech_duration_s":60.0,"speech_pad_ms":400,"samples_overlap_s":null}
}
```

- **Parent:** `000-original`
- **Condition:** run only if D2's `speaker.lift` held above 6.0× **and** D1→D2 showed monotone improvement in `dels` or `subs`.
- **Hypothesis:** the asymptote of the text benefit and, most likely, the point where segmentation collapses — 800 ms of merge window will absorb most conversational pauses.
- **Falsifier:** if D3 improves `dels`/`subs` further *without* costing `lift`, my reading of the merge rule at `whisper.cpp:5383` is wrong and the whole granularity-vs-fidelity tension I have asserted does not exist. That would be a significant correction to the ledger.

---

### Block E — `min_silence_duration_ms`. Is segment shape a *text* axis or only a usability axis?

Why this axis and not another: a **null result here closes a whole region of the search space.** Whisper sees the same concatenated audio either way — only where the concatenation seams fall changes. If `subs` really is invariant to that, then every remaining segmentation knob can be settled on editor taste with zero further GPU time.

---

**E1 — `min_silence_duration_ms` → 300**

```json
{
  "audio": {"resample_chunk":4096,"sinc_len":256,"f_cutoff":0.95,"oversampling_factor":256,"interpolation":"linear"},
  "decode": {"language":"en","beam_size":5,"patience":1.0,"entropy_thold":3.0,"logprob_thold":null,"no_speech_thold":null,"temperature":null,"temperature_inc":null,"no_context":true,"suppress_nst":null,"initial_prompt":null,"max_len":null,"split_on_word":null,"token_timestamps":true},
  "vad": {"enabled":true,"threshold":null,"min_speech_duration_ms":null,"min_silence_duration_ms":300,"max_speech_duration_s":60.0,"speech_pad_ms":null,"samples_overlap_s":null}
}
```

- **Parent:** `000-original`
- **Hypothesis:** a speech run must now be followed by 300 ms of silence to end, up from 100. Segments fall from 417 toward ~320; `mean_segment_tokens` up from 12.0 toward ~15.5; `bleed` up from 16 per the ledger's coarser-means-more-bleed finding on 026; `lift` roughly unchanged, since it is normalised against chance. **`subs` and `dels` flat.**
- **Falsifier:** `subs` moves by ≥ 10 in either direction → segment shape *is* a text axis, every VAD knob is also a decoding knob, and E must be escalated above blocks G/I/J/K.

---

**E2 — `min_silence_duration_ms` → 700**

```json
{
  "audio": {"resample_chunk":4096,"sinc_len":256,"f_cutoff":0.95,"oversampling_factor":256,"interpolation":"linear"},
  "decode": {"language":"en","beam_size":5,"patience":1.0,"entropy_thold":3.0,"logprob_thold":null,"no_speech_thold":null,"temperature":null,"temperature_inc":null,"no_context":true,"suppress_nst":null,"initial_prompt":null,"max_len":null,"split_on_word":null,"token_timestamps":true},
  "vad": {"enabled":true,"threshold":null,"min_speech_duration_ms":null,"min_silence_duration_ms":700,"max_speech_duration_s":60.0,"speech_pad_ms":null,"samples_overlap_s":null}
}
```

- **Parent:** `000-original`
- **Hypothesis:** the extreme rung. Segments fall toward ~220, `mean_segment_tokens` toward ~22, `bleed` up sharply. Still `subs`/`dels` flat.
- **Falsifier:** `subs` within ±6 of 63 at **both** E1 and E2 → segmentation is confirmed cosmetic with respect to text. Close the text question for the whole VAD-shape family and defer E, F and J to a usability decision made off the GPU. That is the outcome I expect and the one worth having.

---

### Block F — `max_len` / `split_on_word`. Pure structure, and a free check on the instrument.

Why this axis and not another: `whisper_wrap_segment` (`whisper.cpp:6042`) is post-processing over an already-decoded token list. It **cannot** change a token. So this block has a rare property — a sharp, source-derived prediction that the text metrics are *exactly* invariant. If they move, either my reading is wrong or the harness's text assembly is sensitive to segmentation, and the latter would invalidate every cross-config text comparison where segment counts differ, which is all of blocks D, E, J and K. That makes F1 a correctness check on the instrument that costs one run.

Note the chain structure: F2's parent is F1 and F3's parent is F2, because `split_on_word` is inert while `max_len` is unset (finding (d)) — proposing `split_on_word: true` against `000-original` would be a guaranteed no-op run.

---

**F1 — `max_len` → 80**

```json
{
  "audio": {"resample_chunk":4096,"sinc_len":256,"f_cutoff":0.95,"oversampling_factor":256,"interpolation":"linear"},
  "decode": {"language":"en","beam_size":5,"patience":1.0,"entropy_thold":3.0,"logprob_thold":null,"no_speech_thold":null,"temperature":null,"temperature_inc":null,"no_context":true,"suppress_nst":null,"initial_prompt":null,"max_len":80,"split_on_word":null,"token_timestamps":true},
  "vad": {"enabled":true,"threshold":null,"min_speech_duration_ms":null,"min_silence_duration_ms":null,"max_speech_duration_s":60.0,"speech_pad_ms":null,"samples_overlap_s":null}
}
```

- **Parent:** `000-original`
- **Hypothesis:** segments rise from 417 to roughly 550–650 (baseline `mean_segment_tokens` is 12.0, so most segments already sit near 60–70 characters and only the long tail wraps). `mean_segment_tokens` falls toward ~9. `speaker.covered` rises but `speaker.lift` **falls**, since finer segmentation is the trivially-gamed direction the ledger warns about. **`wer.rate`, `subs`, `dels`, `ins_*` and `lexicon.recall` all exactly unchanged.**
- **Falsifier:** any movement at all in `subs` or `dels` → either `whisper_wrap_segment` is not text-preserving, or the harness's segment-to-token assembly is segmentation-sensitive. **Either would be an instrument defect that must be resolved before any block D/E/J/K result is believed.** Escalate to `eval-harness` immediately and treat that as the round's finding.

---

**F2 — `split_on_word` → true (parent F1)** · *conditional*

```json
{
  "audio": {"resample_chunk":4096,"sinc_len":256,"f_cutoff":0.95,"oversampling_factor":256,"interpolation":"linear"},
  "decode": {"language":"en","beam_size":5,"patience":1.0,"entropy_thold":3.0,"logprob_thold":null,"no_speech_thold":null,"temperature":null,"temperature_inc":null,"no_context":true,"suppress_nst":null,"initial_prompt":null,"max_len":80,"split_on_word":true,"token_timestamps":true},
  "vad": {"enabled":true,"threshold":null,"min_speech_duration_ms":null,"min_silence_duration_ms":null,"max_speech_duration_s":60.0,"speech_pad_ms":null,"samples_overlap_s":null}
}
```

- **Parent:** **F1** (one axis relative to F1, not to `000-original`)
- **Condition:** run only if F1's text metrics were invariant as predicted. If they were not, the instrument is broken and this block is on hold.
- **Hypothesis:** wraps now fall only where a token begins with a space (`whisper.cpp:6026`), so no segment starts mid-word. Segment count falls slightly from F1 (some wrap points are deferred), `mean_segment_tokens` rises slightly. Text metrics still exactly unchanged. The real deliverable is qualitative: segments become usable as caption cues.
- **Falsifier:** segment count identical to F1 → no wrap point in the whole file landed mid-word, `split_on_word` is inert on this content, and F3 is pointless.

---

**F3 — `max_len` → 45 (parent F2)** · *conditional*

```json
{
  "audio": {"resample_chunk":4096,"sinc_len":256,"f_cutoff":0.95,"oversampling_factor":256,"interpolation":"linear"},
  "decode": {"language":"en","beam_size":5,"patience":1.0,"entropy_thold":3.0,"logprob_thold":null,"no_speech_thold":null,"temperature":null,"temperature_inc":null,"no_context":true,"suppress_nst":null,"initial_prompt":null,"max_len":45,"split_on_word":true,"token_timestamps":true},
  "vad": {"enabled":true,"threshold":null,"min_speech_duration_ms":null,"min_silence_duration_ms":null,"max_speech_duration_s":60.0,"speech_pad_ms":null,"samples_overlap_s":null}
}
```

- **Parent:** **F2**
- **Condition:** run only if F2 changed segment count relative to F1, and only if budget survives.
- **Hypothesis:** roughly a single caption line. Segments roughly double from F2; `speaker.lift` degrades further toward the chance floor. This rung exists to locate where lift collapses, which bounds the usable caption-length range.
- **Falsifier:** `speaker.lift` at F3 is not below F2's → lift is insensitive to `max_len`, meaning the post-hoc wrap does not disturb the VAD-derived boundaries that generate it. That would actually be good news (captions and diarization are decoupled) and is worth recording either way.

---

### Block G — `beam_size`. Is the decoder search-limited at all?

Why this axis and not another: G1 is the cheapest **null-closes-the-block** probe available. Beam 2 runs faster than beam 5, so it costs less than a full slot.

---

**G1 — `beam_size` → 2**

```json
{
  "audio": {"resample_chunk":4096,"sinc_len":256,"f_cutoff":0.95,"oversampling_factor":256,"interpolation":"linear"},
  "decode": {"language":"en","beam_size":2,"patience":1.0,"entropy_thold":3.0,"logprob_thold":null,"no_speech_thold":null,"temperature":null,"temperature_inc":null,"no_context":true,"suppress_nst":null,"initial_prompt":null,"max_len":null,"split_on_word":null,"token_timestamps":true},
  "vad": {"enabled":true,"threshold":null,"min_speech_duration_ms":null,"min_silence_duration_ms":null,"max_speech_duration_s":60.0,"speech_pad_ms":null,"samples_overlap_s":null}
}
```

- **Parent:** `000-original`
- **Hypothesis:** if the decoder is search-limited, halving the beam should cost 5–15 substitutions. If it is not, `subs` barely moves and the entire decode-search block is low-yield. Wall clock down ~30 %.
- **Falsifier:** `subs` within ±5 of 63 → the decoder is not search-limited on this content, **skip G2**, and deprioritise search-widening for the rest of the campaign.
- **Why 2 and not 1:** `n_decoders = max(greedy.best_of, beam_size)` with `greedy.best_of = -1` under the beam-search strategy (`whisper.cpp:5959`, `6872`), so `beam_size: 1` yields a single-decoder beam search — a degenerate path with some crash risk. A crashed run wastes a slot for no information. Beam 2 gives the same directional answer safely.

---

**G2 — `beam_size` → 8** · *conditional*

```json
{
  "audio": {"resample_chunk":4096,"sinc_len":256,"f_cutoff":0.95,"oversampling_factor":256,"interpolation":"linear"},
  "decode": {"language":"en","beam_size":8,"patience":1.0,"entropy_thold":3.0,"logprob_thold":null,"no_speech_thold":null,"temperature":null,"temperature_inc":null,"no_context":true,"suppress_nst":null,"initial_prompt":null,"max_len":null,"split_on_word":null,"token_timestamps":true},
  "vad": {"enabled":true,"threshold":null,"min_speech_duration_ms":null,"min_silence_duration_ms":null,"max_speech_duration_s":60.0,"speech_pad_ms":null,"samples_overlap_s":null}
}
```

- **Parent:** `000-original`
- **Condition:** run only if G1 showed the decoder *is* search-limited (`subs` up by ≥ 5 at beam 2).
- **Hypothesis:** completes the 2/5/8 ladder upward. Expect a smaller gain than G1's loss — beam search saturates. `subs` down 2–5; wall clock up ~50 % to ~400 s, which is a real cost against the round-2 budget.
- **Falsifier:** `subs` down by less than 3 → beam 5 is already at the saturation point, and beam width should be left alone for the remainder of the campaign. Given the cost, this is a bad trade even at the margin.

---

### Block K — `vad.threshold`. Silero's speech/non-speech decision.

Why this axis and not another: given finding (e), this is the knob that decides what audio *exists*. It is more consequential than pad or silence-duration but also blunter, so it runs after them.

---

**K1 — `vad.threshold` → 0.35 (more permissive)**

```json
{
  "audio": {"resample_chunk":4096,"sinc_len":256,"f_cutoff":0.95,"oversampling_factor":256,"interpolation":"linear"},
  "decode": {"language":"en","beam_size":5,"patience":1.0,"entropy_thold":3.0,"logprob_thold":null,"no_speech_thold":null,"temperature":null,"temperature_inc":null,"no_context":true,"suppress_nst":null,"initial_prompt":null,"max_len":null,"split_on_word":null,"token_timestamps":true},
  "vad": {"enabled":true,"threshold":0.35,"min_speech_duration_ms":null,"min_silence_duration_ms":null,"max_speech_duration_s":60.0,"speech_pad_ms":null,"samples_overlap_s":null}
}
```

- **Parent:** `000-original`
- **Hypothesis:** more audio survives into the filtered buffer — quiet speech, trailing words, the interviewer's off-mic backchannel. `dels` down from 31; `hyp_tokens` up; `ins_content` up as room tone and backchannel get transcribed. Segment count up.
- **Falsifier:** `dels` within ±4 of 31 **and** `hyp_tokens` up by less than 40 → 0.5 is not clipping real speech on this recording and the permissive direction is closed.

---

**K2 — `vad.threshold` → 0.65 (more conservative)**

```json
{
  "audio": {"resample_chunk":4096,"sinc_len":256,"f_cutoff":0.95,"oversampling_factor":256,"interpolation":"linear"},
  "decode": {"language":"en","beam_size":5,"patience":1.0,"entropy_thold":3.0,"logprob_thold":null,"no_speech_thold":null,"temperature":null,"temperature_inc":null,"no_context":true,"suppress_nst":null,"initial_prompt":null,"max_len":null,"split_on_word":null,"token_timestamps":true},
  "vad": {"enabled":true,"threshold":0.65,"min_speech_duration_ms":null,"min_silence_duration_ms":null,"max_speech_duration_s":60.0,"speech_pad_ms":null,"samples_overlap_s":null}
}
```

- **Parent:** `000-original`
- **Hypothesis:** the mirror rung, and the one that makes K interpretable — a two-sided ladder around the default separates "0.5 is miscalibrated" from "the metric is insensitive to threshold". Expect `dels` up from 31, `hyp_tokens` down, `ins_content` down. If `ins_content` falls a lot while `dels` barely rises, the archive gets a cleaner transcript for free.
- **Falsifier:** both K1 and K2 land within ±4 `dels` and ±40 `hyp_tokens` of baseline → Silero's decision is insensitive across 0.35–0.65 on this recording, and `threshold` is closed for the corpus. Conversely, if K1 and K2 move in the *same* direction on `dels`, the axis is re-rolling rather than acting and no causal claim survives.

---

### Block J — `max_speech_duration_s`. The other silent non-default.

Why this axis and not another: `60.0` is ours; whisper.cpp's default is `FLT_MAX`. 108 is characterised in the ledger as "long reflective answers", so a 60 s hard ceiling is forcing breaks into exactly the content type this interview is made of. Lower priority than D/E only because it interacts with `speech_pad_ms` (`whisper.cpp:5215`: the effective cap is `sample_rate × max_speech_duration_s − n_window − 2 × speech_pad_samples`), so its reading is cleanest once D has reported.

---

**J1 — `max_speech_duration_s` → 120.0**

```json
{
  "audio": {"resample_chunk":4096,"sinc_len":256,"f_cutoff":0.95,"oversampling_factor":256,"interpolation":"linear"},
  "decode": {"language":"en","beam_size":5,"patience":1.0,"entropy_thold":3.0,"logprob_thold":null,"no_speech_thold":null,"temperature":null,"temperature_inc":null,"no_context":true,"suppress_nst":null,"initial_prompt":null,"max_len":null,"split_on_word":null,"token_timestamps":true},
  "vad": {"enabled":true,"threshold":null,"min_speech_duration_ms":null,"min_silence_duration_ms":null,"max_speech_duration_s":120.0,"speech_pad_ms":null,"samples_overlap_s":null}
}
```

- **Parent:** `000-original`
- **Hypothesis:** forced breaks halve in number. Since whisper.cpp splits at the *nearest silence* rather than an arbitrary frame, the removed breaks were the least-natural ones. Expect a small `subs` improvement (0–5) and slightly longer segments. Mostly this is a check that 60 s is not doing harm.
- **Falsifier:** `subs` within ±4 and segment count within ±20 of 417 → the 60 s cap is rarely binding on this interview (mean segment is 4.3 s median, so it plausibly almost never fires), and J is closed. **Skip J2.**

---

**J2 — `max_speech_duration_s` → 30.0** · *conditional*

```json
{
  "audio": {"resample_chunk":4096,"sinc_len":256,"f_cutoff":0.95,"oversampling_factor":256,"interpolation":"linear"},
  "decode": {"language":"en","beam_size":5,"patience":1.0,"entropy_thold":3.0,"logprob_thold":null,"no_speech_thold":null,"temperature":null,"temperature_inc":null,"no_context":true,"suppress_nst":null,"initial_prompt":null,"max_len":null,"split_on_word":null,"token_timestamps":true},
  "vad": {"enabled":true,"threshold":null,"min_speech_duration_ms":null,"min_silence_duration_ms":null,"max_speech_duration_s":30.0,"speech_pad_ms":null,"samples_overlap_s":null}
}
```

- **Parent:** `000-original`
- **Condition:** run only if J1 showed the cap is binding (segment count moved by more than 20).
- **Hypothesis:** the downward rung, forcing roughly twice as many hard breaks. Establishes whether J is monotone or whether 60 happens to sit near an optimum.
- **Falsifier:** J2 lands on the same side of baseline as J1 → not monotone, axis is re-rolling, close it.

---

### Block I — `samples_overlap_s`. A specific, testable source of insertions.

**I1 — `samples_overlap_s` → 0.0**

```json
{
  "audio": {"resample_chunk":4096,"sinc_len":256,"f_cutoff":0.95,"oversampling_factor":256,"interpolation":"linear"},
  "decode": {"language":"en","beam_size":5,"patience":1.0,"entropy_thold":3.0,"logprob_thold":null,"no_speech_thold":null,"temperature":null,"temperature_inc":null,"no_context":true,"suppress_nst":null,"initial_prompt":null,"max_len":null,"split_on_word":null,"token_timestamps":true},
  "vad": {"enabled":true,"threshold":null,"min_speech_duration_ms":null,"min_silence_duration_ms":null,"max_speech_duration_s":60.0,"speech_pad_ms":null,"samples_overlap_s":0.0}
}
```

- **Parent:** `000-original`
- **Why this axis:** `whisper.cpp:6658-6660` appends `samples_overlap × 16000` samples of the *next* segment onto the end of each segment. At the default 0.1 s that is 1600 samples of duplicated audio per boundary × ~417 boundaries. Duplicated audio is a mechanically plausible insertion source, and it is the only one in the plan with a precisely identifiable count.
- **Hypothesis:** setting it to 0 removes ~417 short duplications. If boundary duplication is producing text, `ins_content` falls measurably (10–30) and `hyp_tokens` falls with it. Risk in the other direction: the overlap exists to stop words being cut at seams, so `dels` may rise.
- **Falsifier:** `ins_content` within ±10 of 272 **and** `dels` within ±4 of 31 → 0.1 s is too short to produce a whole token and the axis is inert. Close it. (This is my most likely null, but the mechanism is specific enough to be worth one run, and it is the only remaining VAD field untested after D, E, K and J.)

---

### Block H — `no_speech_thold`. Lowest priority; included only for completeness.

**H1 — `no_speech_thold` → 0.3** · *conditional, run last or not at all*

```json
{
  "audio": {"resample_chunk":4096,"sinc_len":256,"f_cutoff":0.95,"oversampling_factor":256,"interpolation":"linear"},
  "decode": {"language":"en","beam_size":5,"patience":1.0,"entropy_thold":3.0,"logprob_thold":null,"no_speech_thold":0.3,"temperature":null,"temperature_inc":null,"no_context":true,"suppress_nst":null,"initial_prompt":null,"max_len":null,"split_on_word":null,"token_timestamps":true},
  "vad": {"enabled":true,"threshold":null,"min_speech_duration_ms":null,"min_silence_duration_ms":null,"max_speech_duration_s":60.0,"speech_pad_ms":null,"samples_overlap_s":null}
}
```

- **Parent:** `000-original`
- **Hypothesis:** `no_speech_thold` gates two things — the fallback trigger (`whisper.cpp:7555`) and `is_no_speech`, which causes a whole window's text to be **discarded** (`whisper.cpp:7585`, guarded at `7603`). Lowering it to 0.3 makes discard *more* likely. I expect `dels` to rise. This is a probe of whether whole-window drops are happening at all.
- **Falsifier:** `dels` within ±4 of 31 → no window is anywhere near the discard boundary, and both `no_speech_thold` and `logprob_thold` are closed together (they gate the same conjunction).
- **Honest priority note:** `dels` is 31 and the deletion taxonomy is single function words, not blocks. There is no evidence in the baseline that any window is being dropped. I am listing H1 because it is the last unexplored decode field, **not** because I expect it to move — and I would cut it first if the budget tightens. If your instinct is to drop it, drop it.

---

## 7. Things I am explicitly NOT proposing, and why

| Not proposed | Reason |
|---|---|
| `decode.initial_prompt` | Out of scope per your constraint. |
| Anything in `audio.*` | Resampler axis closed by Phase 0: 0.9978 NCC, zero residual drift across all eight windows. |
| `decode.patience` | **Dead knob.** Read the field in `whisper.cpp` — it appears only in the struct and two default initialisers, and is never consulted by the decode loop. Any run varying it is guaranteed null. |
| `decode.split_on_word` against `000-original` | **Provable no-op.** Inert unless `max_len > 0` (`whisper.cpp:6059`). Only meaningful as F2, parented on F1. |
| `decode.suppress_nst` | No non-speech tokens anywhere in the baseline taxonomy. Insertions are `and`/`i`/`you know`/`um`. Proposing it would be padding. |
| `decode.token_timestamps` | Product requirement (DTW word timing for the editor), and disabling it would silently disable `max_len` too. |
| `decode.temperature` | Only meaningful jointly with `temperature_inc`; block B tests the mechanism more cleanly with one axis. |
| `decode.logprob_thold` | Gates the same conjunction as `no_speech_thold` (`whisper.cpp:7555`, `7585`). H1 tests the conjunction; a second run on the other half is redundant until H1 shows the conjunction is live. |
| `vad.min_speech_duration_ms` | Default 250 ms. Would drop short interviewer interjections — plausible, but block K's threshold sweep covers the same "what survives VAD" question more directly, and I would rather run K two-sided than split the budget. Round 2 if K implicates VAD. |
| `vad.enabled: false` | Not a tuning experiment — it would change segmentation, timing and text simultaneously, and the ledger's central structural finding (8.96× speaker lift) is *produced by* VAD. If you want it, it is a baseline question needing its own pair of runs. |
| Any run on 026 / 074 / `108_truth.*` | Held back as validation. Round 2, after round 1 names a winner. |

---

## 8. What round 1 is designed to produce

Not a winning config — a **map**. By the end you should be able to say, from the ledger rather than from memory:

1. Whether GPU runs are byte-reproducible, and therefore what a delta is worth (R0, cross-checked by B1).
2. Whether the temperature-fallback ladder is firing, and whether `entropy_thold: 3.0` was a mistake (A + B, with A4 as positive control and A3↔B1 as an independent cross-check).
3. Whether context priming is the usable substitute for the prompt you have excluded (C).
4. Whether VAD boundaries are clipping real content, and where the fidelity-vs-segmentation frontier sits (D + K).
5. **Whether segment shape is a text axis or only a usability axis** (E + F1) — the answer that determines how much of the remaining search space needs GPU time at all.

If blocks A, B, C and D all come back null, that is not a failed round. It means the baseline config is already at a local optimum on everything reachable without the prompt, and the honest recommendation at that point is to stop tuning decode parameters and spend the remaining budget on validating `000-original` across 026 and 074 instead. I would say so rather than manufacture a round 2.

---

**Files referenced:**
- `/home/ubuntu/auohp/docs/transcription-tuning-log.md`
- `/home/ubuntu/auohp/packages/core/src/transcription/config.rs`
- `/home/ubuntu/auohp/packages/core/src/transcription/whisper.rs` (param application, lines 136–213)
- `/home/ubuntu/auohp/packages/core/src/eval/metrics.rs` (scorecard fields)
- `/home/ubuntu/auohp/packages/core/src/bin/transcribe.rs`
- `/mnt/s3/fs1/out/runs/000-original/score.json`, `/mnt/s3/fs1/out/runs/000-original/run.log`
- `/home/ubuntu/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/whisper-rs-sys-0.15.0/whisper.cpp/src/whisper.cpp` (findings a–e)

Nothing was run and the ledger was not touched.agentId: a707bcf70b3cbd54b (use SendMessage with to: 'a707bcf70b3cbd54b', summary: '<5-10 word recap>' to continue this agent)
<usage>subagent_tokens: 93794
tool_uses: 39
duration_ms: 620984</usage>