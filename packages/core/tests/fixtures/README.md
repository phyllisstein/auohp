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

The `scripts/build_*` scripts regenerate the derived fixtures from the source
transcripts. Each asserts that concatenating the turns reproduces the cleaned
truth **token-for-token**, so `clean.txt` and `turns.json` cannot silently drift
apart — a drift there would corrupt the diarization metric without erroring.

Worth running the printed counts against the source every time. On 047 the tag
count was seven short of `grep -c` on the raw file: a tape marker landing
immediately before a speaker tag defeated the `^` anchor, and seven turns were
silently glued onto their predecessors. Nothing errored, and the transcript still
read correctly — only the turn boundaries were wrong, which is precisely the
input the diarization metric trusts.

## Coverage — the fixtures do not all mean the same thing

| Interview | Truth covers | Media |
|---|---|---|
| 108 Avram Finkelstein | a 34-minute excerpt | 34 min clip, plus the 3.4 h master |
| 026 Iris Long | ~2× the clip's tape | 34 min clip |
| 074 Douglas Crimp | pages 37–43 only | 34 min clip |
| **047 Jim Eigo** | **the whole recording** | **2.61 h, original and re-encode** |

047 is the first fixture where truth and media are the same span, so it is the
only one where `partial_coverage` should come back false and WER is computed over
the entire interview rather than an overlap. It also carries 32 tape markers
across four tapes against 108's six, and `score_drift` fits each tape separately —
four independent slope estimates over 2.6 hours, by a distance the best timing
evidence in the corpus.

It is deliberately **not** in `tests/decode.rs`. That table wants an
externally-resampled WAV as its control, and decoding three 2.6-hour files would
turn a 47-second CPU test into a multi-minute, multi-gigabyte one. The decode path
is settled on 108 and 026 at clip length.

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

### 047: blank lines inside turns

047 sits between the two cases. Like 026 the speaker tag is alone on its line with
the text following after a blank; like 074 nothing is reordered, so no
reconstruction is needed. The trap is that blank lines appear *within* turns as
well as between them, so a blank cannot mark a turn boundary — only a tag line
starts a turn. The opening exchange is tagged with full names before the
transcript settles into initials, so the tag pattern accepts both.
