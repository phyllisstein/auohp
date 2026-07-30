"""Build the 074 fixtures (Douglas Crimp, interviewed by Sarah Schulman).

Unlike 026, this extraction keeps each speaker tag inline with its text and never
reorders lines, so no layout reconstruction is needed. What it does carry is
running headers, page numbers, and tape markers wedged mid-sentence.
"""
import re, json, textwrap, pathlib
from collections import Counter

SRC = "/mnt/s3/fs1/in/074_text.txt"
FIX = pathlib.Path("/home/ubuntu/auohp/packages/core/tests/fixtures")
SENT = "\x00M%d\x00"

stats = Counter()
raw = [l.rstrip() for l in open(SRC, encoding="utf-8")]

FURNITURE = [
    (r"^Douglas Crimp Interview$", "running header"),
    (r"^May 16, 2007$", "running header"),
    (r"^\d{1,3}$", "page number"),
]
TAPE = re.compile(r"^Tape\s+(I|II|III|IV)$")
TIME = re.compile(r"^(\d\d):(\d\d):(\d\d)$")
ROMAN = {"I": 1, "II": 2, "III": 3, "IV": 4}

turns, markers = [], []
pending_tape, pending_mark = None, None

for line in raw:
    s = line.strip()
    if not s:
        continue
    if TAPE.match(s):
        pending_tape = ROMAN[TAPE.match(s).group(1)]
        stats["tape marker"] += 1
        continue
    m = TIME.match(s)
    if m and pending_tape:
        pending_mark = len(markers)
        markers.append({"tape": pending_tape,
                        "label": f"Tape {'I' * pending_tape} {m[0]}",
                        "tape_seconds": int(m[1]) * 3600 + int(m[2]) * 60 + int(m[3])})
        pending_tape = None
        continue
    hit = next((why for pat, why in FURNITURE if re.match(pat, s)), None)
    if hit:
        stats[hit] += 1
        continue

    chunk = s
    if pending_mark is not None:
        chunk = SENT % pending_mark + " " + chunk
        pending_mark = None

    tag = re.match(r"^(SS|DC|JH|JW)\s*:\s*(.*)$", chunk)
    if tag:
        stats["speaker tags"] += 1
        turns.append({"speaker": "SUBJECT" if tag.group(1) == "DC" else "INTERVIEWER",
                      "text": tag.group(2)})
    elif turns:
        turns[-1]["text"] += " " + chunk          # ordinary wrapping
    else:
        turns.append({"speaker": "INTERVIEWER", "text": chunk})

for t in turns:
    # Leading en-dashes mark a question resumed after an interjection; they are
    # punctuation, not speech, and `normalize` would drop them anyway.
    t["text"], n = re.subn(r"\[[^\]]*\]\s*", "", t["text"]); stats["editorial insertions"] += n
    t["text"] = re.sub(r"\s+", " ", t["text"]).strip()
turns = [t for t in turns if t["text"].replace(" ", "")]

flat = " ".join(t["text"] for t in turns)
for i, mk in enumerate(markers):
    j = flat.find(SENT % i)
    after = re.sub(r"\x00M\d+\x00", " ", flat[j + len(SENT % i):]) if j >= 0 else ""
    mk["following_text"] = " ".join(after.split()[:14])
for t in turns:
    t["text"] = re.sub(r"\s*\x00M\d+\x00\s*", " ", t["text"]).strip()
text = " ".join(t["text"] for t in turns)

FIX.joinpath("074_truth.clean.txt").write_text(
    "\n".join(textwrap.wrap(text, 96, break_on_hyphens=False, break_long_words=False)) + "\n")
json.dump({"_comment": [
    "Speaker turns for 074 (Douglas Crimp, interviewed by Sarah Schulman, 16 May 2007).",
    "Concatenating `text` reproduces 074_truth.clean.txt token-for-token; asserted at build."],
    "turns": turns}, open(FIX / "074_truth.turns.json", "w"), indent=1)
json.dump({"_comment": [
    "Tape markers from the PDF margin. `tape_seconds` is time WITHIN that tape.",
    "The transcript is an excerpt (pages 37-43), so it may cover less tape than the",
    "clip, or more; the harness reports `partial_coverage` either way."],
    "anchors": markers}, open(FIX / "074_truth.anchors.json", "w"), indent=1)

assert " ".join(t["text"] for t in turns).split() == text.split()
print(f"words {len(text.split())}   turns {len(turns)}   markers {len(markers)}")
print("speakers:", Counter(t["speaker"] for t in turns))
for k, v in sorted(stats.items()):
    if v: print(f"  {v:4}  {k}")
