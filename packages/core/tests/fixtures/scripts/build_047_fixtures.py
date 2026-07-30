"""Build the 047 fixtures (Jim Eigo, interviewed by Sarah Schulman, 5 March 2004).

Layout sits between the two cases already handled. Like 026 the speaker tag is
alone on its line with the text following after a blank; like 074 nothing is
reordered, so no column reconstruction is needed. The wrinkle here is that blank
lines appear *inside* turns as well as between them, so a blank cannot be treated
as a turn boundary --- only a tag line starts a turn.

The first two turns are tagged with full names before the transcript settles into
initials, which is why the tag pattern accepts both.

This is the first fixture whose transcript covers the *whole* recording rather
than an excerpt, and the first with all four tapes marked: 32 markers, eight per
tape, against six for 108. `score_drift` fits per tape, so that is four
independent slope estimates over 2.6 hours --- by a distance the best timing
evidence in the corpus.
"""
import re, json, textwrap, pathlib
from collections import Counter

SRC = "/mnt/s3/fs1/in/047_truth.txt"
FIX = pathlib.Path("/home/ubuntu/auohp/packages/core/tests/fixtures")
SENT = "\x00M%d\x00"

stats = Counter()
raw = [l.rstrip() for l in open(SRC, encoding="utf-8")]

FURNITURE = [
    (r"^Jim Eigo Interview$", "running header"),
    (r"^March 5, 2004$", "running header"),
    (r"^\d{1,3}$", "page number"),
    (r"^\[END OF INTERVIEW\]$", "end marker"),
]
# Full names for the opening exchange, initials thereafter. `$` matters: a bare
# tag line is the turn boundary, and `SS: text` never occurs in this extraction.
TAG = re.compile(r"^(?:(SS|JE)|(SARAH SCHULMAN|JIM EIGO))\s*:\s*(.*)$")
SUBJECT = {"JE", "JIM EIGO"}
TAPE = re.compile(r"^Tape\s+(I|II|III|IV)$")
TIME = re.compile(r"^(\d\d):(\d\d):(\d\d)$")
ROMAN = {"I": 1, "II": 2, "III": 3, "IV": 4}

turns, markers = [], []
pending_tape, pending_mark = None, None
started = False

for line in raw:
    s = line.strip()
    if not s:
        continue

    # Front matter runs until the first speaker tag; it carries a copyright
    # notice and a festival credit that would otherwise land in turn one.
    if not started:
        if not TAG.match(s):
            stats["front matter"] += 1
            continue
        started = True

    if TAPE.match(s):
        pending_tape = ROMAN[TAPE.match(s).group(1)]
        stats["tape marker"] += 1
        continue
    m = TIME.match(s)
    if m and pending_tape:
        pending_mark = len(markers)
        markers.append({"tape": pending_tape,
                        "label": f"Tape {'I' * pending_tape if pending_tape < 4 else 'IV'} {m[0]}",
                        "tape_seconds": int(m[1]) * 3600 + int(m[2]) * 60 + int(m[3])})
        pending_tape = None
        continue
    hit = next((why for pat, why in FURNITURE if re.match(pat, s)), None)
    if hit:
        stats[hit] += 1
        continue

    # Match the tag against the bare line, never against a sentinel-prefixed one.
    # A tape marker sitting immediately before a tag would otherwise defeat the
    # `^` anchor, and the turn would be silently glued onto its predecessor ---
    # seven of them, before this was caught by counting tags against turns.
    tag = TAG.match(s)
    if tag:
        who = tag.group(1) or tag.group(2)
        body = tag.group(3)
        if pending_mark is not None:
            body = (SENT % pending_mark + " " + body).strip()
            pending_mark = None
        stats["speaker tags"] += 1
        turns.append({"speaker": "SUBJECT" if who in SUBJECT else "INTERVIEWER",
                      "text": body})
        continue

    chunk = s
    if pending_mark is not None:
        chunk = SENT % pending_mark + " " + chunk
        pending_mark = None

    if turns:
        turns[-1]["text"] += " " + chunk          # ordinary wrapping
    else:
        turns.append({"speaker": "INTERVIEWER", "text": chunk})

for t in turns:
    t["text"], n = re.subn(r"\[[^\]]*\]\s*", "", t["text"]); stats["editorial insertions"] += n
    t["text"] = re.sub(r"\s+", " ", t["text"]).strip()

flat = " ".join(t["text"] for t in turns)
for i, mk in enumerate(markers):
    j = flat.find(SENT % i)
    after = re.sub(r"\x00M\d+\x00", " ", flat[j + len(SENT % i):]) if j >= 0 else ""
    mk["following_text"] = " ".join(after.split()[:14])
for t in turns:
    t["text"] = re.sub(r"\s*\x00M\d+\x00\s*", " ", t["text"]).strip()
# Only now: a turn carrying nothing but a marker sentinel looks non-empty until
# the sentinel is gone.
turns = [t for t in turns if t["text"]]
text = " ".join(t["text"] for t in turns)

FIX.joinpath("047_truth.clean.txt").write_text(
    "\n".join(textwrap.wrap(text, 96, break_on_hyphens=False, break_long_words=False)) + "\n")
json.dump({"_comment": [
    "Speaker turns for 047 (Jim Eigo, interviewed by Sarah Schulman, 5 March 2004).",
    "Concatenating `text` reproduces 047_truth.clean.txt token-for-token; asserted at build."],
    "turns": turns}, open(FIX / "047_truth.turns.json", "w"), indent=1)
json.dump({"_comment": [
    "Tape markers from the PDF margin. `tape_seconds` is time WITHIN that tape,",
    "so the four tapes are four coordinate systems; `score_drift` fits each",
    "separately. Unlike the other three fixtures this transcript covers the whole",
    "recording, so `partial_coverage` should be false on a full-length run."],
    "anchors": markers}, open(FIX / "047_truth.anchors.json", "w"), indent=1)

assert " ".join(t["text"] for t in turns).split() == text.split()
assert all(m["following_text"] for m in markers), "an anchor lost its following text"
print(f"words {len(text.split())}   turns {len(turns)}   markers {len(markers)}")
print("speakers:", Counter(t["speaker"] for t in turns))
print("markers per tape:", Counter(m["tape"] for m in markers))
for k, v in sorted(stats.items()):
    if v: print(f"  {v:4}  {k}")
