#!/usr/bin/env python3
"""Flag-only cleanup spike --- reconnaissance for a local-LLM "sharp tool".

Point an LLM at ONE transcript and have it FLAG likely-misrecognised spans
(names, orgs, technical terms) for a human editor. It must NOT rewrite, propose
replacements, or assert anything. Flag only --- machine points, human types.

Eyeball the flags against the real errors found in the gold comparison.

  python3 flag_spike.py [interview_number]   # default 027 (a CLEAN transcript)
"""

import re
import requests
import sys
import json
from pathlib import Path
from dataclasses import dataclass, asdict

ASSETS = Path(__file__).parent.parent / "assets"
NUM = sys.argv[1] if len(sys.argv) > 1 else "027"  # use a CLEAN transcript only


@dataclass
class Word:
    word: str
    p: float


@dataclass
class Flag:
    text: str  # suspect text, copied verbatim from the transcript
    reason: str  # one word: name | org | term | garbled
    words: list[Word]


@dataclass
class Segment:
    text: str
    words: list[Word]


def transcript_segments(num: str) -> list[Segment]:
    data = json.loads((ASSETS / f"{num}.wav.json").read_text())
    segments = []
    for segment in data.get("segments", []):
        segment_words = segment.get("words", [])
        words = []
        for word in segment_words:
            if not re.search(r"\[", word["word"]):
                w = Word(word=word["word"], p=word["p"])
                words.append(w)
        segments.append(Segment(text=segment["text"], words=words))

    return segments


def transcript_text(num: str) -> str:
    segments = transcript_segments(num)
    return " ".join(s.text for s in segments)


def uncertain_segments(segments: list[Segment], max_p: float = 0.05):
    uncertain = []

    for s in segments:
        for idx, w in enumerate(s.words):
            if w.p <= max_p:
                left = max(idx - 3, 0)
                right = min(idx + 4, len(s.words))
                uncertain_words = s.words[left:right]
                suspect = idx - left  # the low-p word's position WITHIN the window
                suspect_word = uncertain_words[suspect]
                uncertain_words[suspect] = Word(
                    p=suspect_word.p, word=f"«{suspect_word.word}»"
                )
                # TODO(human): build `uncertain_text` from `uncertain_words`, but
                # MARK the word at `suspect` so the LLM judges THAT word, not the
                # fragment's grammar. e.g. wrap it -> "the «debates» in ACT".
                uncertain_text = " ".join(w.word for w in uncertain_words)
                uncertain_segment = Segment(text=uncertain_text, words=uncertain_words)
                uncertain.append(uncertain_segment)
                break

    return uncertain


def flag_spans(segments: list[Segment]) -> list[Flag]:
    uncertain = uncertain_segments(segments)

    r = requests.post(
        "http://localhost:11434/api/chat",
        headers={"content-type": "application/json"},
        data=json.dumps(
            {
                # "model": "gemma4:e2b",
                "model": "qwen3.5:2b",
                "stream": False,
                "think": False,
                "options": {
                    "temperature": 0,
                },
                "messages": [
                    {
                        "role": "system",
                        "content": "These transcribed texts include a word Whisper was unsure of (low p value per word). The uncertain word in each text segment is surrounded with « » characters. Judge only the marked word. Is it a plausible mishearing? If the transcription is likely correct in context, omit the segment from the JSON output.",
                    },
                    {
                        "role": "system",
                        "content": "Consider, in particular, technical language and terminology specific to AIDS activism and ACT UP.",
                    },
                    {
                        "role": "system",
                        "content": 'Comply with the following JSON schema: `{"segments": [{"text": complete text of the suspect segment, "words": [{"word": individual suspect word from the segment, "p": p value of the word as transcribed}], "reason": reason for considering the text inaccurate]}`',
                    },
                    {
                        "role": "system",
                        "content": "The 'reason' field should identify the justification for flagging the text as erroneous: 'name' for an incorrect transcription of a person's name; 'org' for an incorrectly transcribed organization; 'term' for an incorrectly transcribed language specifc to the domain; and 'garbled' for generally poorly parsed or misheard text which is incorrect in the transcription.",
                    },
                    {
                        "role": "system",
                        "content": "The output message content should be raw machine-readable JSON",
                    },
                    {
                        "role": "user",
                        "content": json.dumps([asdict(s) for s in uncertain]),
                    },
                ],
                "format": "json",
            },
        ),
    )
    response_data = r.json()
    content = response_data.get("message", {}).get("content", "{}")
    # content = sanitize_content(content)
    content = json.loads(content)
    print(json.dumps(content, indent=4))

    flags = []
    for c in content.get("segments", []):
        text = c.get("text")
        words = c.get("words")
        reason = c.get("reason")

        flags.append(Flag(text=text, reason=reason, words=words))

    return flags


def sanitize_content(raw_content: str) -> str:
    sanitized = re.sub(r"```(json)?", "", raw_content)
    return sanitized


def main() -> None:
    flags = flag_spans(transcript_segments(NUM))
    print(f"{len(flags)} flags:")
    for f in flags:
        print(f"  [{str(f.reason):7}] {f.text}")


if __name__ == "__main__":
    main()
