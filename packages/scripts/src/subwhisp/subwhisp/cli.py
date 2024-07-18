from pathlib import Path
import click

from .subber import (
    to_json_captions,
    to_vtt_captions
)
from .whisperer import transcribe_audio_file

@click.group()
def run():
    pass

@click.group()
def transcribe():
    pass

@transcribe.command()
@click.argument('input_file')
def json(input_file):
    basename = Path(input_file).stem
    whisper_transcription = transcribe_audio_file(input_file)

    json_captions = to_json_captions(whisper_transcription)

    with open(f'{basename}.json', 'w') as f:
        f.write(json.dumps(json_captions))


@transcribe.command()
@click.argument('input_file')
def vtt(input_file):
    basename = Path(input_file).stem
    whisper_transcription = transcribe_audio_file(input_file)

    vtt_captions = to_vtt_captions(whisper_transcription)

    with open(f'{basename}.vtt', 'w') as f:
        f.write(vtt_captions)


run.add_command(transcribe)
