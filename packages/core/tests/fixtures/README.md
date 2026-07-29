# Scoring fixtures

Per interview, four files drive `bin/score`:

| File | Role |
|---|---|
| `<id>_truth.clean.txt` | Cleaned human transcript. The reference for WER and lexicon recall. |
| `<id>_truth.turns.json` | Speaker turns. Drives `structure.speaker` — whether segment boundaries respect speaker changes. |
| `<id>_truth.anchors.json` | Tape markers. Drives timing-drift reporting. |
| `actup_lexicon.txt` | Shared across interviews. Terms absent from a given truth are skipped, so corpus-wide entries cost nothing. |

Media is gigabytes and lives outside the repo — under `AUOHP_FIXTURE_DIR`,
`/mnt/s3/fs1/in`, or `$HOME`. Tests needing it skip with a notice.

## Rebuilding

`scripts/build_026_fixtures.py` and the `build_108_*` scripts regenerate the
derived fixtures from the source transcripts. Each asserts that concatenating the
turns reproduces the cleaned truth **token-for-token**, so `clean.txt` and
`turns.json` cannot silently drift apart — a drift there would corrupt the
diarization metric without erroring.

## Why the cleaning is not a regex pass

Both transcripts are PDF extractions with layout artifacts, and each needs a
different treatment. The rules that matter:

- **Strip what was not spoken**: running headers, page numbers, tape markers,
  stage directions, and bracketed editorial insertions (keep `Nesline`, drop
  `[Michael]` — the model transcribes speech, and the name was not said).
- **Preserve everything else verbatim.** Do not lowercase, expand contractions,
  or fix the speaker's grammar. `eval::normalize` handles all of that at compare
  time, and doing it twice by two sets of rules is how the two sides end up
  disagreeing for reasons that have nothing to do with the model.
- **Repair damage only where the intent is unambiguous** (`hi erar chai` →
  `hierarchical`). Where it is not, leave it and say so — inventing a reading
  fabricates ground truth, which is worse than a known-noisy token.

### 026: reconstructing column layout

026's PDF was authored in Word with the speaker tag in a left column and the text
in a right one. `pdftotext` emits each turn's *first* line in reading order but
defers the wrapped remainder, so continuations resurface later, sometimes after
an intervening speaker:

```
SS:  How did ACT UP respond to this information? What did they decide
IL:  They essentially accepted it and put it on their agenda. That was it. It was
     to do?          <- belongs to SS
     on the agenda.  <- belongs to IL
```

Two signals recover the order:

- **A blank line separates layout blocks.** Blank-separated text that is not the
  line immediately following a tag is a *displaced* continuation.
- **Contiguous lines are ordinary wrapping** and stay where they are.

Displaced lines refill the oldest turn still ending mid-sentence — FIFO. That one
rule resolved all 84 displacements, including `names.` landing back on a turn
three exchanges earlier. Tape markers are captured as sentinels in the stream and
their `following_text` read off the *assembled* transcript, because raw file
order is exactly what the displacement makes unreliable.
