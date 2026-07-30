"""Build the 026 fixtures from the pdftotext extraction.

The PDF is a Word document with the speaker tag in a left column and the text in
a right one. pdftotext emits each turn's *first* line in reading order but defers
the wrapped remainder, so continuations surface later -- sometimes after an
intervening speaker. Two signals recover the original order:

  * A blank line separates layout blocks. Blank-separated text that is not the
    line right after a tag is a *displaced* continuation.
  * Contiguous lines (no blank between) are ordinary wrapping and stay put.

Displaced lines refill the oldest turn still ending mid-sentence -- FIFO. That
single rule resolves every case in this transcript, including "names." landing
back on a turn three exchanges earlier.
"""
import re, json, textwrap, pathlib
from collections import Counter

SRC = "/mnt/s3/fs1/in/026_truth.txt"
FIX = pathlib.Path("/home/ubuntu/auohp/packages/core/tests/fixtures")
SENT = "\x00M%d\x00"

raw = [l.rstrip() for l in open(SRC, encoding="utf-8")]
stats = Counter()

FURNITURE = [
    (r"^Iris Long Interview$", "running header"),
    (r"^May 16, 2003$", "running header"),
    (r"^\d{1,3}$", "page number"),
]
TAPE = re.compile(r"^Tape\s+(I|II|III|IV)$")
TIME = re.compile(r"^(\d\d):(\d\d):(\d\d)$")
ROMAN = {"I": 1, "II": 2, "III": 3, "IV": 4}

turns, markers = [], []
cur_speaker, pending_tag, pending_tape, pending_mark = None, False, None, None
prev_blank = True

def open_turn_index():
    """Oldest turn whose text still ends mid-sentence."""
    for i, t in enumerate(turns):
        if not re.search(r"[.?!–\"']\s*$", t["text"]):
            return i
    return None

for line in raw:
    s = line.strip()
    if not s:
        prev_blank = True
        continue

    if re.fullmatch(r"(SS|IL|JW):", s):
        cur_speaker, pending_tag = s[:2], True
        stats["speaker tags"] += 1
        prev_blank = False
        continue

    if TAPE.match(s):
        pending_tape = ROMAN[TAPE.match(s).group(1)]
        stats["tape marker"] += 1
        prev_blank = False
        continue
    m = TIME.match(s)
    if m and pending_tape:
        secs = int(m[1]) * 3600 + int(m[2]) * 60 + int(m[3])
        pending_mark = len(markers)
        markers.append({"tape": pending_tape,
                        "label": f"Tape {'I'*pending_tape} {m[0]}",
                        "tape_seconds": secs})
        pending_tape = None
        prev_blank = False
        continue

    hit = next((why for pat, why in FURNITURE if re.match(pat, s)), None)
    if hit:
        stats[hit] += 1
        prev_blank = False
        continue

    # A sentinel marks where a tape marker interrupted the flow, so the anchor's
    # following text can be read off the assembled transcript rather than guessed
    # from file order -- which the displacement makes unreliable.
    chunk = s
    if pending_mark is not None:
        chunk = SENT % pending_mark + " " + chunk
        pending_mark = None

    if pending_tag:
        turns.append({"speaker": "SUBJECT" if cur_speaker == "IL" else "INTERVIEWER",
                      "text": chunk})
        pending_tag = False
    elif not prev_blank and turns:
        turns[-1]["text"] += " " + chunk            # ordinary wrapping
    else:
        idx = open_turn_index()
        if idx is None:
            idx = len(turns) - 1                     # nothing open: current turn
        else:
            stats["displaced lines refiled"] += 1
        turns[idx]["text"] += " " + chunk
    prev_blank = False

# --- repairs ----------------------------------------------------------------
REPAIR = [(r"\btrails\b", "trials")]                 # typed typo, not OCR
for t in turns:
    for pat, rep in REPAIR:
        t["text"], n = re.subn(pat, rep, t["text"]); stats[f"repair {pat}"] += n
    t["text"], n = re.subn(r"\[[^\]]*\]\s*", "", t["text"]); stats["editorial insertions"] += n
    t["text"] = re.sub(r"\s+", " ", t["text"]).strip()
turns = [t for t in turns if t["text"].replace(" ", "")]

# --- extract anchor following-text, then strip sentinels ---------------------
flat = " ".join(t["text"] for t in turns)
for i, mk in enumerate(markers):
    tag = SENT % i
    j = flat.find(tag)
    after = re.sub(r"\x00M\d+\x00", " ", flat[j + len(tag):]) if j >= 0 else ""
    mk["following_text"] = " ".join(after.split()[:14])
for t in turns:
    t["text"] = re.sub(r"\s*\x00M\d+\x00\s*", " ", t["text"]).strip()
text = " ".join(t["text"] for t in turns)

FIX.joinpath("026_truth.clean.txt").write_text(
    "\n".join(textwrap.wrap(text, 96, break_on_hyphens=False, break_long_words=False)) + "\n")
json.dump({"_comment": [
    "Speaker turns for 026 (Iris Long, interviewed by Sarah Schulman, 16 May 2003).",
    "Concatenating `text` reproduces 026_truth.clean.txt token-for-token; asserted at build.",
    "Rebuilt from pdftotext output whose column layout defers wrapped lines; see build026.py."],
    "turns": turns}, open(FIX / "026_truth.turns.json", "w"), indent=1)
json.dump({"_comment": [
    "Tape markers from the PDF margin. `tape_seconds` is time WITHIN that tape;",
    "Tape II restarts its clock, so the harness fits each tape separately.",
    "`following_text` was read off the assembled transcript at the marker's position,",
    "not from raw file order, which the column layout makes unreliable.",
    "The clip is shorter than the transcript, so late anchors may report NOT FOUND."],
    "anchors": markers}, open(FIX / "026_truth.anchors.json", "w"), indent=1)

assert " ".join(t["text"] for t in turns).split() == text.split()
print(f"words {len(text.split())}   turns {len(turns)}   markers {len(markers)}")
print("speakers:", Counter(t["speaker"] for t in turns))
for k, v in sorted(stats.items()):
    if v: print(f"  {v:4}  {k}")
