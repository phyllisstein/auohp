"""
Normalize splits between subtitle segments based on natural-language processing.

Adapted from GitHub user @ankitgurua:
    https://gist.github.com/ankitgurua/7b0db06baa8e2c7288cbbf396169120d
See context in WhisperX issue:
    https://github.com/m-bain/whisperX/issues/829
"""


import os
import argparse
import logging
import json
import re
from more_itertools import chunked
from itertools import pairwise
from collections.abc import Iterator

import spacy
from spacy.language import Language
from spacy.tokens import Doc, Span, Token
from spacy.matcher import Matcher
from spacy.cli.download import download as spacy_download

def format_timestamp(seconds: float, always_include_hours: bool = False, decimal_marker: str = '.', milliseconds: bool = False):
    assert seconds >= 0, "non-negative timestamp expected"
    milliseconds = round(seconds * 1000.0)

    hours = milliseconds // 3_600_000
    milliseconds -= hours * 3_600_000

    minutes = milliseconds // 60_000
    milliseconds -= minutes * 60_000

    seconds = milliseconds // 1_000
    milliseconds -= seconds * 1_000

    hours_marker = f"{hours:02d}:" if always_include_hours or hours > 0 else ""
    milliseconds_marker = f"{decimal_marker}{milliseconds:03d}" if milliseconds else ""
    return f"{hours_marker}{minutes:02d}:{seconds:02d}{milliseconds_marker}"

def get_time_span(span: Span, timing: dict):
    start_token = span[0]
    end_token = span[-1]
    while start_token.is_punct or not timing.get(start_token.idx, None):
        start_token = start_token.nbor(-1)
    while end_token.is_punct or not timing.get(end_token.idx, None):
        end_token = end_token.nbor(-1)
    end_index = end_token.idx
    start_index = start_token.idx
    start_time, _ = timing[start_index]
    _, end_time = timing.get(end_index, (None, None))
    if not end_time:
        logging.debug("Timing alignment error: %s %d", span.text, end_token.idx)
    return (start_time, end_time)

def get_speaker(span: Span, speakers: dict):
    start_token = span[-1]

    while not speakers.get(start_token.idx, None):
        try:
            start_token = start_token.nbor(-1)
        except:
            return "NO_SPEAKER"

    return speakers.get(start_token.idx, "SPEAKER_00")

Token.set_extension("can_fragment_after", default=False)
Token.set_extension("fragment_reason", default="")
Span.set_extension("get_time_span", method=get_time_span)
Span.set_extension("get_speaker", method=get_speaker)

punct_pattern = [{'IS_PUNCT': True, 'ORTH': {"IN": [",", ":", ";"]}}]
conj_pattern = [{"POS": {"IN": ["CCONJ", "SCONJ"]}}]
clause_pattern = [{"DEP": {"IN": ["advcl", "relcl", "acl", "acl:relcl"]}}]
ac_comp_pattern = [{"DEP": {"IN": ["acomp", "ccomp"]}}]
preposition_pattern = [{'POS': 'ADP'}]
dobj_pattern = [{'DEP': 'dobj'}, {'IS_PUNCT': False}]
v_particle_pattern = [{'POS': 'VERB'}, {'POS': 'PART'}, {'POS': {"IN": ["VERB", "AUX"]}, 'OP': '!'}]
v_adj_pattern =  [{'POS': "VERB"}, {"POS": "ADJ", "DEP": "amod"}]

@Language.factory("fragmenter", default_config={"verbal_pauses": []})
def create_fragmenter_component(nlp: Language, name: str, verbal_pauses: list[int]):
    return FragmenterComponent(nlp, verbal_pauses)

class FragmenterComponent:
    def __init__(self, nlp: Language, verbal_pauses: list):
        self.pauses = set(verbal_pauses)
        logging.info("Count of pauses: %d", len(self.pauses))

    def __call__(self, doc: Doc) -> Doc:
        return fragmenter(doc, self.pauses)

def _fragment_at(token: Token, reason: str):
    token._.can_fragment_after = True
    token._.fragment_reason = reason

def fragmenter(doc: Doc, pauses: set) -> Doc:
    matcher = Matcher(doc.vocab)
    matcher.add("clause", [clause_pattern])
    matcher.add("punct", [punct_pattern])
    matcher.add("conj", [conj_pattern])
    matcher.add("preposition", [preposition_pattern])
    matcher.add("dobj", [dobj_pattern])
    matcher.add("v_particle", [v_particle_pattern])
    matcher.add("ac_comp", [ac_comp_pattern])
    matcher.add("v_adj", [v_adj_pattern])

    matches = matcher(doc)
    conjunction_or_punct = frozenset(["CCONJ", "SCONJ", "PUNCT"])
    for match_id, start, end in matches:
        rule_id = doc.vocab.strings[match_id]
        matched_span = doc[start:end]
        token = doc[start]
        if token.i < 2:
            continue
        match rule_id:
            case "punct":
                _fragment_at(token, reason=rule_id)
            case "conj":
                prior = token.nbor(-1)
                if prior.pos_ not in conjunction_or_punct:
                    _fragment_at(prior, reason=rule_id)
            case "clause":
                subtree = [t for t in token.subtree]
                if len(subtree) < 2:
                    continue
                clause_rule = f"{rule_id}:{token.text}"
                left = subtree[0]
                if left and left.i > 0 and not left.is_punct and left.text[0] != "'":
                    prior = left.nbor(-1)
                    if prior.i > 0 and prior.pos_ not in conjunction_or_punct and not prior.nbor(-1).is_punct:
                        _fragment_at(prior, reason=clause_rule)
                right = subtree[-1]
                try:
                    if right.pos_ not in conjunction_or_punct and not right.nbor(1).is_punct:
                        _fragment_at(right, reason=clause_rule)
                except IndexError:
                    continue
            case "preposition":
                prior = token.nbor(-1)
                if prior.pos_ in conjunction_or_punct or token.ent_iob_ == 'I':
                    continue
                next = token.nbor(1)
                if (token.dep_ == 'prt' or prior.pos_ in ['AUX', 'VERB']) and not next.is_punct:
                    _fragment_at(token, reason=f"{rule_id}-after")
                else:
                    if token.i > 2 and not (prior.is_punct or prior.nbor(-1).is_punct):
                        _fragment_at(prior, reason=rule_id)
            case "v_particle":
                particle = matched_span[1]
                if particle.is_punct or particle.nbor(1).is_punct:
                    continue
                _fragment_at(particle, reason=rule_id)
            case "v_adj":
                _fragment_at(token, reason=rule_id)
            case "dobj":
                if token.pos_ not in conjunction_or_punct:
                    _fragment_at(token, reason=rule_id)
            case "ac_comp":
                if token.is_punct:
                    continue
                subtree = [t for t in token.subtree]
                left = subtree[0]
                if len(subtree) < 2:
                    continue
                ac_rule = f"{rule_id}:{token.text}"
                if left and left.i > 0 and left.text[0] != "'" and not (left.is_punct or left.nbor(-1).is_punct):
                    _fragment_at(left.nbor(-1), reason=ac_rule)
                right = subtree[-1]
                try:
                    if not (right.is_punct or right.nbor(1).is_punct):
                        _fragment_at(right, reason=ac_rule)
                except IndexError:
                    logging.debug("ac_comp IndexError")
                    continue
    _scan_entities(doc)
    _scan_noun_phrases(doc)
    _scan_pauses(doc, pauses)
    return doc

def _scan_pauses(doc: Doc, pauses: set):
    for token in doc:
        if token.text[0] == '-':
            continue
        try:
            if token.idx in pauses and not token.nbor(1).is_punct:
                logging.debug("Candidate pause: %d %s %s", token.i, token.text, token.nbor(1).text)
        except IndexError:
            continue

def _scan_entities(doc: Doc):
    for entity in doc.ents:
        if len(entity) < 2 or entity.label_ in ['PERSON', 'ORDINAL', 'PERCENT', 'TIME', 'CARDINAL'] or len(entity.text) < 10:
            continue
        token = entity[0]
        if token.i < 1:
            continue
        prior = token.nbor(-1)
        if (not prior.is_punct and
            prior.pos_ not in ['DET'] and
            not prior._.can_fragment_after):
            _fragment_at(prior, reason="entity->")
        after = entity[-1].nbor(1)
        if (after.pos_ != "PART" and
            not after.is_punct and
            not entity[-1]._.can_fragment_after):
            _fragment_at(entity[-1], reason="entity")

def _scan_noun_phrases(doc: Doc):
    for chunk in doc.noun_chunks:
        if len(chunk) < 2:
            continue
        token = chunk[0]
        if token.i > 0 and not token.is_punct:
            prior = token.nbor(-1)
            if (prior.pos_ not in ['ADP', 'SCONJ', 'CCONJ'] and
                not prior._.can_fragment_after and
                not prior.is_punct):
                _fragment_at(prior, reason="NP->")
        try:
            after = chunk[-1].nbor(1)
            if (not after.is_punct and
                not chunk[-1]._.can_fragment_after):
                _fragment_at(chunk[-1], reason="NP")
        except IndexError:
            continue

def load_whisper_json(jsdata: dict) -> tuple[str, dict, dict]:
    doc_timing = {}
    doc_speakers = {}
    doc_text = ""
    for s in jsdata:
        if 'words' not in s:
            raise ValueError('JSON input file must contain word timestamps')
        for word_timed in s['words']:
            word = word_timed['word']
            if len(doc_text) == 0:
                word = word.lstrip()
                start_index = 0
            doc_text += word + " "
            start_index = len(doc_text) - len(word) - 1

            # FIXME: Digits in transcript JSON do not have timings.
            if 'start' in word_timed and 'end' in word_timed:
                doc_timing[start_index] = (word_timed['start'], word_timed['end'])

            if 'speaker' in word_timed:
                doc_speakers[start_index] = word_timed['speaker']

    return doc_text.strip(), doc_timing, doc_speakers

def scan_for_pauses(doc_text: str, timing: dict) -> list[int]:
    pauses = []
    for (k1, (_, end)), (k2, (start, _)) in pairwise(sorted(timing.items())):
        gap = start - end
        if gap > 0.3:
            pauses.append(k1)
    return pauses

def preferred_division_for(span: Span, max_width: int) -> int:
    def is_grammatically_preferred(token: Token):
        return token._.can_fragment_after and (
            token._.fragment_reason in ['punct', 'conj', 'clause', 'entity', 'entity->', 'v_adj'])
    preferreds = (t for t in reversed(span) if is_grammatically_preferred(t))
    target_width = round(0.7 * max_width)
    for tp in preferreds:
        width = tp.idx + len(tp) - span.start_char
        if width > max_width:
            continue
        remainder_width = span.end_char - tp.idx - len(tp)
        if width <= remainder_width and width >= max_width/3 and remainder_width <= max_width:
            logging.debug("Primary complete %s %s at %d : '%s'", tp.text, tp._.fragment_reason, tp.idx - span.start_char, span.text)
            return tp.i
        if width >= target_width and width <= remainder_width * 1.2:
            logging.debug("Primary selected %s %s at %d : '%s'", tp.text, tp._.fragment_reason, tp.idx - span.start_char, span.text)
            return tp.i
    logging.debug("No primary for '%s'", span.text)
    return 0

def secondary_division_for(span: Span, max_width: int) -> int:
    token_divider = 0
    start_index = span.start_char
    for token in span:
        if token.i == span[0].i:
            continue
        token_start = token.idx - start_index
        if token_divider and token_start > max_width:
            break
        token_end = token_start + len(token)
        if token._.can_fragment_after and token_end <= max_width and token.i + 2 < span[-1].i:
            token_divider = token.i
            if span.end_char - token.idx - len(token) <= max_width:
               break
        if not token_divider and token_end > max_width:
            token_divider = token.i - 1 if token.pos_ != 'PUNCT' else token.i - 2
            logging.info("Forced division after word '%s' : '%s'", span.doc[token_divider].text, span.text)
    return token_divider

def divide_span(span: Span, **args) -> Iterator[Span]:
    max_width = args.get("width", 42)
    if span.end_char - span.start_char <= max_width:
        yield span
        return
    divider = preferred_division_for(span, max_width) or secondary_division_for(span, max_width)
    after_divider = divider + 1
    yield span.doc[span.start:after_divider]
    if after_divider < span.end:
        yield from divide_span(span.doc[after_divider:span.end], **args)

def iterate_document(doc: Doc, timing: dict, speakers: dict, **args):
    max_lines = args.get("lines", 2)
    for sentence in doc.sents:
        for chunk in chunked(divide_span(sentence, **args), max_lines):
            subtitle = '\n'.join(line.text for line in chunk)
            sub_start, _ = chunk[0]._.get_time_span(timing)
            _, sub_end = chunk[-1]._.get_time_span(timing)
            speaker = chunk[0]._.get_speaker(speakers)
            yield sub_start, sub_end, subtitle, speaker

def write_vtt(doc, timing, **args):
    comma: str = '.'
    vtt = f"WEBVTT\n\n"
    for i, (start_time, end_time, text, speaker) in enumerate(iterate_document(doc, timing, {}, **args), start=1):
        ts1 = format_timestamp(start_time, always_include_hours=True, decimal_marker=comma, milliseconds=True)
        ts2 = format_timestamp(end_time, always_include_hours=True, decimal_marker=comma, milliseconds=True)

        vtt += f"{ts1} --> {ts2}\n{text}\n\n"
    return vtt

def write_json(doc, timing, speakers, **args):
    segments = []
    for i, (start_time, end_time, text, speaker) in enumerate(iterate_document(doc, timing, speakers, **args), start=1):
        start_timestamp = format_timestamp(start_time, always_include_hours=False)
        end_timestamp = format_timestamp(end_time, always_include_hours=False)
        segments.append({
            "startTime": start_time,
            "endTime": end_time,
            "transcription": text,
            "speaker": speaker,
            "startTimestamp": start_timestamp,
            "endTimestamp": end_timestamp,
            "type": "statement",
            "children": [{"text": text}]
        })
    return [
        {
            "type": "transcript",
            "children": segments
        }
    ]

def configure_spaCy(model: str, pauses: list = []):
    if not spacy.util.is_package(model):
        spacy_download(model)
    nlp = spacy.load(model)
    if model.startswith('xx'):
        raise NotImplementedError("spaCy multilanguage models are not currently supported")
    nlp.add_pipe("fragmenter", config={"verbal_pauses": pauses}, last=True)
    return nlp

def main():
    parser = argparse.ArgumentParser(
                prog='subwisp',
                description='Convert a whisper .json transcript into .srt subtitles with sentences, grammatically separated where possible.',
                epilog='')
    parser.add_argument('input_file')
    parser.add_argument('-m', '--model', help='specify spaCy model', default="en_core_web_lg")
    parser.add_argument('-e', '--entities', help='optional custom entities for spaCy (.jsonl format)', default="")
    parser.add_argument('-w', '--width', help='maximum line width', default=60, type=int)
    parser.add_argument('-l', '--lines', help='maximum lines per subtitle', default=2, type=int, choices=range(1,4))
    parser.add_argument('-d', '--debug', help='print debug information',
                        action="store_const", dest="loglevel", const=logging.DEBUG, default=logging.WARNING)
    parser.add_argument('--verbose', help='be verbose',
                        action="store_const", dest="loglevel", const=logging.INFO)

    args = parser.parse_args()
    logging.basicConfig(level=args.loglevel)
    if not os.path.isfile(args.input_file):
        logging.error("File not found: %s", args.input_file )
        exit(-1)
    if not args.model:
        logging.error("No spacy model specified")
        exit(-1)
    if len(args.entities) > 0 and not os.path.isfile(args.entities):
        logging.error("Entities file not found: %s", args.entities)
        exit(-1)

    with open(args.input_file) as f:
        jsdata = json.load(f)

    wtext, word_timing, word_speakers = load_whisper_json(jsdata)
    verbal_pauses = scan_for_pauses(wtext, word_timing)
    nlp = configure_spaCy(args.model, args.entities, verbal_pauses)
    doc = nlp(wtext)

    # write_vtt(doc, word_timing, args)
    write_json(doc, word_timing, word_speakers, args)
    exit()


def to_json_captions(input: dict, model: str = "en_core_web_sm", width: int = 42, lines: int = 2):
    wtext, word_timing, word_speakers = load_whisper_json(input)
    verbal_pauses = scan_for_pauses(wtext, word_timing)
    nlp = configure_spaCy(model, verbal_pauses)
    doc = nlp(wtext)
    return write_json(doc, word_timing, word_speakers, width=width, lines=lines)


def to_vtt_captions(input: dict, model: str = "en_core_web_sm", width: int = 42, lines: int = 2):
    wtext, word_timing, word_speakers = load_whisper_json(input)
    verbal_pauses = scan_for_pauses(wtext, word_timing)
    nlp = configure_spaCy(model, verbal_pauses)
    doc = nlp(wtext)
    return write_vtt(doc, word_timing, width=width, lines=lines)

def to_captions(input: dict, model: str = "en_core_web_sm", width: int = 42, lines: int = 2):
    wtext, word_timing, word_speakers = load_whisper_json(input)
    verbal_pauses = scan_for_pauses(wtext, word_timing)
    nlp = configure_spaCy(model, verbal_pauses)
    doc = nlp(wtext)
    json = write_json(doc, word_timing, word_speakers, width=width, lines=lines)
    vtt = write_vtt(doc, word_timing, width=width, lines=lines)
    return json, vtt
