import json
import click
import whisperx
import platform
import os

from pathlib import Path
from spacy.cli.download import download as spacy_download

from .subber import (
    to_json_captions,
    to_vtt_captions
)
from .whisperer import transcribe_audio_file


MODELS = os.path.join(os.environ["HOME"], ".subwhisp")

if platform.system() == "Darwin":
    device = "cpu"
    compute_type = "int8"
else:
    device = "cuda"
    compute_type = "float16"


@click.group()
def run():
    pass

@click.command()
@click.argument('input_file')
def transcribe(input_file):
    basename = Path(input_file).stem

    whisper_transcription = transcribe_audio_file(input_file)
    with open(f'{basename}.json', 'w') as f:
        f.write(json.dumps(whisper_transcription))

    json_captions = to_json_captions(whisper_transcription)
    with open(f'{basename}.captions.json', 'w') as f:
        f.write(json.dumps(json_captions))

    vtt_captions = to_vtt_captions(whisper_transcription)
    with open(f'{basename}.captions.vtt', 'w') as f:
        f.write(vtt_captions)


@click.command()
def models():
    whisperx.load_model("large-v2", device=device, compute_type=compute_type, download_root=MODELS, language="en")
    whisperx.load_align_model(language_code="en", device=device, model_dir=MODELS, model_name="facebook/wav2vec2-large-960h-lv60-self")
    whisperx.DiarizationPipeline("pyannote/speaker-diarization-3.1", use_auth_token="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", device=device)
    spacy_download("en_core_web_lg")


run.add_command(transcribe)
run.add_command(models)
