# Corpus reconnaissance

Ad-hoc tools for asking questions of the 174 ACT UP Oral History PDFs without
reading them. The corpus is ~2.65M words. The constraint that shaped everything
here: answer questions by returning *counts*, so the prose never enters context.

Built 2026-08-26 while looking for hand-tagging candidates to seed a demo.
Kept because the dead ends are more useful than the results.

## Usage

```sh
./extract.sh                              # one-time: PDFs -> ./cache
./rank-topic.sh terms-stop-the-church.txt
./affinity-groups.sh
```

## What's here

| File | Does |
|---|---|
| `extract.sh` | PDFs to a two-form text cache (line-oriented + flattened) |
| `rank-topic.sh` | Rank transcripts by tier-1 term hits |
| `affinity-groups.sh` | Group rosters + multi-group bridges |
| `normalize.awk` | Strip page furniture, attribute speakers, renumber |
| `timecode.awk` | Map every line to an absolute interview timestamp |
| `terms-*.txt` | Tiered search vocabularies |
| `affinity-groups.txt` | The nine group names found so far |

`normalize.awk` and `timecode.awk` are not wired into any script. They belong to
the abandoned approach below and are kept because the timecode reconstruction is
worth having when the statement-level model needs real timings.

## Findings

**Topic top tens, by tier-1 hit count.** Regenerated 2026-08-27 from a rebuilt
cache; `rank-topic.sh` reproduces both.

| # | Stop the Church | | Political funerals | |
|---|---|---|---|---|
| 1 | 064 Vincent Gagliostro | 19 | 085 BC Craig | 25 |
| 2 | 174 Paul O'Dwyer | 16 | 036 Joy Episalla | 18 |
| 3 | 123 Elizabeth Meixell | 15 | 028 Richard Deagle | 13 |
| 4 | 037 Jamie Leo | 13 | 032 Ron Goldberg | 10 |
| 5 | 148 Tony Arena & Ron Grunewald | 11 | 093 Risa Denenberg | 8 |
| 6 | 149 Tracy Morgan | 10 | 021 Russell Pritchard | 7 |
| 7 | 130 John Voelcker | 10 | 121 Donna Binder | 6 |
| 8 | 188 Kathy Ottersten | 8 | 144 Eugene Fedorko | 5 |
| 9 | 163 Lori Cohen | 8 | 074 Douglas Crimp | 5 |
| 10 | 099 Betty Williams | 8 | 062 Stephen Shapiro | 5 |

`rank-topic.sh` counts per *file*, and interview 099 is two files: Betty Williams
was interviewed twice (2008-08-08 and 2008-08-23), and the first session is
misfiled as `089 - Donald Grove.pdf`. Each scores 8; together 16, which would
place her second. The table above is per-file and does not do that merge. Any
future ranking should collapse on `interview_number` from the manifest, not on
filename---099 is the only duplicated number in the corpus, but it is enough to
move a top-three slot.

The Stop the Church tail is a five-way tie at 8 hits: Ottersten, Cohen, both
Williams files, and 050 Peter Cramer, who falls outside the top ten only because
`head -10` cuts the tie arbitrarily.

Read the tails skeptically. Political funerals has tier-1 anchors in only 19 of
174 transcripts, and ranks 8--10 are a three-way tie at five hits---below that
line the ordering is arbitrary. The top four in each column are the real signal.

**The two metrics disagree, and that is the interesting part.** The abandoned
duration pass (dead end 2) ranked Stop the Church as Gagliostro 18.2 min,
Blotcher 8.9, Wolfe 8.7, Meixell 8.3. Only Gagliostro and Meixell survive into
the hit-count list; Blotcher and Wolfe do not place at all, and O'Dwyer arrives
at #2 from nowhere. Some of that gap is the ratcheting bug inflating durations,
but not all of it: hit count rewards repeating a phrase, duration rewards
dwelling on a subject, and an interviewee who says "the cathedral" for ten
minutes after naming it once scores near zero here. Neither number means "this
interview is about the topic"---they are two lossy proxies that happen to
disagree, and where they agree (Gagliostro, Meixell) is worth more than either
ranking alone.

**Nine affinity groups, 76 memberships, 56 people.** Action Tours (12
transcripts), Lavender Hill Mob (16), The Marys (16), Wave 3 (8), Church Ladies
(6), Costas (10), Candelabras (4), Power Tools (3), Awning Leapers (1).

**Articulation points:** Jamie Leo (037), Elizabeth Meixell (123), Jon Winkleman
(131), and Mark Harrington (012) each span three groups. These are the tagging
targets. Harrington matters disproportionately because he is the only bridge out
of Wave 3, which is otherwise its own component.

**Two names surfaced by accident**, from a grammatical-frame pass rather than a
seed list: Awning Leapers and Wizard of Oz. The frame approach mostly returned
determiners (`the affinity group`, `my affinity group`) and was abandoned, but
it found two groups that guessing did not. Worth re-running with better frames
if more groups are wanted.

**Gran Fury and needle exchange are not affinity groups** and are deliberately
absent from `affinity-groups.txt`, despite scoring 322 and 387 mentions. One is
an art collective, the other a campaign. The distinction worth encoding: an
affinity group forms for actions and arrests; a collective produces work.

## Dead end 1: the person co-mention graph

The obvious move is to build `Person -> Person` edges from who names whom, then
show paths. It does not work. The graph is saturated.

Clustering coefficient over the co-mention graph, restricted to distinctive
surnames:

| Person | Degree | Clustering |
|---|---|---|
| Ann Northrop | 23 | 0.54 |
| Peter Staley | 24 | 0.55 |
| Larry Kramer | 23 | 0.60 |
| Joy Episalla | 9 | **1.00** |
| Richard Deagle | 15 | 0.92 |

A coefficient of 1.00 means every neighbor already knows every other neighbor,
so no shortest path routes through that person. Over half the named people
scored 1.00. Any two prominent figures sit 1--2 hops apart via a dozen redundant
routes, which makes a path demo a hairball.

Larry Kramer is the instructive case. Highest degree in the corpus---95 of 173
transcripts name him---but everyone who mentions him already knows each other.
He pools edges without bridging anything. High degree, low betweenness: the
opinion magnet.

Affinity groups fix this precisely because membership is small and bounded.
Wave 3 and Candelabras are nearly disjoint from everything else; Costas leaks
out through exactly two people.

## Dead end 2: measuring minutes instead of mentions

"Which transcripts spend the most *time* on X" is not the same question as
"which have the most hits", and there is real machinery here for answering the
first one. It was built and then judged not worth the complexity for a question
that wanted a rough top ten.

The transcripts carry embedded `HH:MM:SS` timecodes at five-minute intervals,
present in 172 of 174 files, median 19 per interview. They reset to `00:00:00`
at each tape change. `timecode.awk` reconstructs absolute time by detecting
those resets, accumulating a tape offset, and interpolating each line's position
between bracketing markers. It reconstructs 302 hours total, median 101 minutes
per interview---consistent with 2--3 tapes each, which is the sanity check that
says the arithmetic is right.

`normalize.awk` is its prerequisite. These PDFs are double-spaced with page
headers interleaved, so ~52% of raw lines are furniture; normalizing halves the
line count and makes position mean something. It also does sticky speaker
attribution, which falls out of awk's execution model for free---an assigned
variable persists across records, so a two-word state machine fills speaker tags
forward through a turn.

The clustering pass on top of these---group hits into spans, require a tier-1
anchor per cluster, let interviewer turns extend a span without scoring---worked
but produced numbers that needed a diagnostic dump to trust. It is not preserved
here as a runnable script because it had a bug worth remembering rather than
shipping: `sec - cend <= GAP` with `cend` updating on every hit lets a chain of
just-under-threshold mentions ratchet a cluster open indefinitely. Two isolated
mentions reported as an 18-minute span. The ranking it produced still looked
entirely plausible, which is the point: a broken metric that yields a sensible-
looking top ten is harder to catch than one that crashes.

If you resurrect this, the fix is to measure from `cstart` or cap the span, and
to print the hit timeline for the top result before believing any of it.

## Sharp edges

- **Never name a shell variable `GROUPS`.** It is a bash special variable
  holding the invoking user's group IDs. Bash silently refuses to let a script
  assign over it---no error, no warning, `set -u` does not help. The assignment
  looks fine and then every `< "$GROUPS"` reads from a file named `20` (gid
  `staff` on macOS), failing on a redirect line that is perfectly correct. Cost
  most of a debugging session. Same trap applies to `UID`, `PPID`, `RANDOM`,
  `SECONDS`, `LINENO`, `PWD`.
- **BSD grep ignores `-Z` when combined with `-l`** and emits newlines anyway,
  so NUL-delimited filename handling silently receives nothing on macOS. The
  scripts here use `IFS= read -r` over newlines instead, which is safe because
  these filenames contain spaces but never newlines.
- **Delimiters.** The term files were originally `tier|regex`. Regexes contain
  `|`. Everything downstream shredded. They are tab-delimited now; keep it that
  way.
- **Filenames contain spaces.** `NNN - Firstname Lastname.txt`. Use `-print0` /
  `read -d ''`; `xargs` without it will shatter every name.
- **Line-crossing phrases.** Speech wraps constantly, so context-window greps
  need `cache/flat/`, not `cache/txt/`. A grep for `.{70}affinity group.{50}`
  against the line-oriented copy returns nothing.
- **Common-word surnames.** Any per-person count over the manifest is polluted
  by Clear, Brown, Smith, Keith, Charles, Costas. The distinctive-surname list
  used for the clustering table was hand-picked for exactly this reason. A real
  fix wants first-name/surname co-occurrence within a few words.
- **Hit counts are not length-normalized.** A 2.5-hour interview has an edge
  over a 1-hour one. The top of both topic lists is separated widely enough that
  it does not reshuffle, but the tails are within noise.
- **`089 - Donald Grove.pdf` contains the Betty Williams interview.** Known
  issue from the manifest work; it shows up as a duplicate in any per-person
  aggregation.

## Where this points

The affinity group result argues for the statement-level model rather than
person-level: `Person -> Statement -> Event <- Statement <- Person`. The edge
worth walking is not "Deagle mentions Kramer" but "Deagle's account of Tim
Bailey's funeral connects to Episalla's account of the same afternoon". Those
paths are sparse because events are sparse, and they walk through a shared
moment instead of a reputation.

Events with multiple independent witnesses are the tagging target under that
model. The Ashes Action and Tim Bailey's funeral both have the shape: several
people, one afternoon, different vantage points.
