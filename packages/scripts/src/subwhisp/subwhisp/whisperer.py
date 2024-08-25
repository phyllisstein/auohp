import whisperx
import platform
import os
import datetime


MODELS = "/opt/auohp/models"


if platform.system() == "Darwin":
    device = "cpu"
    compute_type = "int8"
else:
    device = "cuda"
    compute_type = "float16"


def transcribe_audio_file(audio_file: str, device: str = device, batch_size: int = 16, compute_type: str = compute_type):
    model = whisperx.load_model("large-v2", device=device, compute_type=compute_type, download_root=MODELS, language="en", asr_options={"suppress_numerals": True})
    audio = whisperx.load_audio(audio_file)
    result = model.transcribe(audio, batch_size=batch_size, language="en")

    model_a, metadata = whisperx.load_align_model(language_code="en", device=device, model_dir=MODELS, model_name="facebook/wav2vec2-large-960h-lv60-self")
    aligned_result = whisperx.align(result["segments"], model_a, metadata, audio, device, return_char_alignments=False)

    diarize_model = whisperx.DiarizationPipeline("pyannote/speaker-diarization-3.1", use_auth_token="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", device=device)
    diarize_segments = diarize_model(audio)

    diarized_result = whisperx.assign_word_speakers(diarize_segments, aligned_result)
    frontend_editor_transcription = []

    for segment in diarized_result["segments"]:
        try:
            startTimestamp = str(datetime.timedelta(seconds=int(segment.get("start", 0))))
            endTimestamp = str(datetime.timedelta(seconds=int(segment.get("end", 0))))
        except:
            startTimestamp = None
            endTimestamp = None

        children = []
        for word in segment["words"]:
            try:
                wordStartTimestamp = str(datetime.timedelta(seconds=int(word.get("start", 0))))
                wordEndTimestamp = str(datetime.timedelta(seconds=int(word.get("end", 0))))
            except:
                wordStartTimestamp = None
                wordEndTimestamp = None

            children.append({
                "start": word.get("start"),
                "end": word.get("end"),
                "startTimestamp": wordStartTimestamp,
                "endTimestamp": wordEndTimestamp,
                "word": word.get("word"),
                "speaker": word.get("speaker"),
                "type": "word",
                "children": [
                    {"text": word.get("word")}
                ]
            })

        frontend_editor_transcription.append({
            "start": segment.get("start"),
            "end": segment.get("end"),
            "startTimestamp": startTimestamp,
            "endTimestamp": endTimestamp,
            "children": children,
            "speaker": segment.get("speaker"),
            "type": "segment"
        })

    frontend_editor_transcription = sorted(frontend_editor_transcription, key=lambda x: x["start"])

    return (diarized_result, frontend_editor_transcription)
