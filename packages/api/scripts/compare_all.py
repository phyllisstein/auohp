#!/usr/bin/env python3
"""Batch quality eval: whisper JSON transcripts vs hand-transcribed gold PDFs.

For every `NNN.wav.json` in the assets dir, find the matching `NNN - Name.pdf`,
extract its text with `pdftotext`, strip ACT UP OHP boilerplate + speaker
labels, normalise both sides, and report an approximate WER plus the genuine
(formatting-stripped) lexical substitution rate.

Bulk text stays in Python; only the summary table is printed. Full per-interview
lexical diffs are written to `NNN.lexical-diffs.txt` for eyeballing.

Run:  python3 compare_all.py [assets_dir]
"""

import re
import sys
import json
import subprocess
from pathlib import Path

ASSETS = (
    Path(sys.argv[1]) if len(sys.argv) > 1 else Path(__file__).parent.parent / "assets"
)

# Speaker labels: ALL-CAPS full names ("SARAH SCHULMAN:") or initials
# ("SS:", "AF:", "RVP:"). 2+ consecutive caps, optionally multi-word, then colon.
_LABEL = re.compile(r"\b[A-Z]{2,}(?:\s+[A-Z]{2,})*:")
# Cover-page / running boilerplate pdftotext lifts out of the transcript body.
_BOILERPLATE = re.compile(
    r"ACT UP Oral History Project|A PROGRAM OF|New York Lesbian|"
    r"Experimental Film Festival|Interviewee:|Interviewer:|"
    r"Interview Number:|Date of Interview:|Interview of ",
    re.IGNORECASE,
)
_PARENS = re.compile(r"[(\[][^)\]]*[)\]]")  # (laughs), [inaudible]
_APOS = re.compile(r"[’‘'`´]")  # any apostrophe variant -> space
_NONWORD = re.compile(r"[^a-z0-9\s]")  # remaining punctuation/dashes -> space
_DIGIT = re.compile(r"\d")

# Disfluency normalisation (METRIC FAIRNESS ONLY -- never touches the archive).
# Whisper transcribes verbatim filler the human transcript cuts; left in, those
# insertions wreck difflib's alignment and masquerade as substitutions. Strip
# them symmetrically from both sides so we measure mis-hearing, not verbatim-ness.
_FILLERS = {"uh", "um", "er", "erm", "mm", "mmm", "hmm", "hm", "ah", "uhhuh", "mmhmm"}
_FILLER_PHRASES = re.compile(r"\b(?:you know|i mean|sort of|kind of)\b")


def extract_gold(pdf: Path) -> str:
    out = subprocess.run(["pdftotext", str(pdf), "-"], capture_output=True, text=True)
    text = out.stdout
    m = _LABEL.search(text)  # drop front matter before first turn
    if m:
        text = text[m.start() :]
    text = "\n".join(ln for ln in text.splitlines() if not _BOILERPLATE.search(ln))
    return _LABEL.sub(" ", text)  # strip the labels themselves


def load_whisper(path: Path) -> str:
    data = json.loads(path.read_text(encoding="utf-8", errors="replace"))
    return " ".join(seg["text"] for seg in data["segments"])


def normalize(text: str) -> list[str]:
    text = text.lower()
    text = _PARENS.sub(" ", text)
    text = _APOS.sub(" ", text)  # wasn't / wasn t -> wasn t (symmetric)
    text = _NONWORD.sub(" ", text)
    text = _FILLER_PHRASES.sub(" ", text)  # drop "you know" / "i mean" tics
    toks = [t for t in text.split() if t not in _FILLERS]
    # Collapse immediate exact repeats ("dance dance" -> "dance"), which under
    # heavy bloat otherwise present as phantom insertions/substitutions.
    out: list[str] = []
    for t in toks:
        if not out or out[-1] != t:
            out.append(t)
    return out


def score(ref, hyp):
    import difflib

    sm = difflib.SequenceMatcher(a=ref, b=hyp, autojunk=False)
    subs = dels = ins = 0
    substitutions = []
    for tag, a0, a1, b0, b1 in sm.get_opcodes():
        if tag == "equal":
            continue
        if tag == "replace":
            subs += max(a1 - a0, b1 - b0)
            substitutions.append((ref[a0:a1], hyp[b0:b1]))
        elif tag == "delete":
            dels += a1 - a0
        elif tag == "insert":
            ins += b1 - b0
    return subs, dels, ins, substitutions


def classify(g, w) -> str:
    if any(_DIGIT.search(t) for t in g + w):
        return "number"
    if "".join(g) == "".join(w):
        return "spacing"
    if len(g) == 1 and len(w) == 1:
        short, lng = sorted((g[0], w[0]), key=len)
        if lng.startswith(short) and len(lng) - len(short) <= 3:
            return "morphology"
    return "lexical"


def find_pairs(assets: Path):
    pdfs = list(assets.glob("*.pdf"))
    for jf in sorted(assets.glob("*.wav.json")):
        num = jf.name.split(".")[0]
        pdf = next((p for p in pdfs if p.name.split(" ")[0] == num), None)
        if pdf:
            name = pdf.stem.split(" - ", 1)[-1]
            yield num, name, jf, pdf


def main() -> None:
    header = (
        f"{'interview':28}{'gold':>7}{'hyp':>7}{'bloat':>7}"
        f"{'rawWER':>8}{'lex':>7}   verdict"
    )
    print(header)
    print("-" * len(header))
    rates = []
    for num, name, jf, pdf in find_pairs(ASSETS):
        ref = normalize(extract_gold(pdf))
        hyp = normalize(load_whisper(jf))
        n = max(len(ref), 1)
        bloat = len(hyp) / n
        label = f"{num} {name}"[:27]
        if len(hyp) > 2 * n:
            print(
                f"{label:28}{len(ref):7}{len(hyp):7}{bloat:7.2f}"
                f"{'--':>8}{'--':>7}   RUNAWAY LOOP"
            )
            continue
        subs, dels, ins, substitutions = score(ref, hyp)
        lex = [(g, w) for g, w in substitutions if classify(g, w) == "lexical"]
        lex_toks = sum(max(len(g), len(w)) for g, w in lex)
        raw = (subs + dels + ins) / n
        lex_rate = lex_toks / n
        rates.append(lex_rate)
        verdict = (
            "under-transcribed"
            if bloat < 0.9
            else "clean"
            if lex_rate < 0.05
            else "degraded"
        )
        print(
            f"{label:28}{len(ref):7}{len(hyp):7}{bloat:7.2f}"
            f"{raw:8.3f}{lex_rate:7.3f}   {verdict}"
        )
        (ASSETS / f"{num}.lexical-diffs.txt").write_text(
            "\n".join(f"{' '.join(g)}\t|\t{' '.join(w)}" for g, w in lex),
            encoding="utf-8",
        )
    if rates:
        print("-" * len(header))
        print(
            f"mean genuine lexical rate across {len(rates)} interviews: "
            f"{sum(rates) / len(rates):.3f}"
        )


if __name__ == "__main__":
    main()
