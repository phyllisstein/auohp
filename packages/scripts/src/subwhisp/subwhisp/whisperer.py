import whisperx
import platform
from .subber import format_timestamp


MODELS = "/opt/auohp/models"


if platform.system() == "Darwin":
    device = "cpu"
    compute_type = "int8"
else:
    device = "cuda"
    compute_type = "float16"


def align_transcription(transcription, audio):
    model_a, metadata = whisperx.load_align_model(language_code="en", device=device, model_dir=MODELS, model_name="facebook/wav2vec2-base-960h")
    aligned_result = whisperx.align(transcription, model_a, metadata, audio, device, return_char_alignments=False)
    return aligned_result


def diarize_audio_file(aligned_result, audio):
    diarize_model = whisperx.DiarizationPipeline("pyannote/speaker-diarization-3.1", use_auth_token="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", device=device)
    diarize_segments = diarize_model(audio)
    diarized_result = whisperx.assign_word_speakers(diarize_segments, aligned_result)
    return diarized_result


def transcribe_audio_file(audio, device: str = device, batch_size: int = 8, compute_type: str = compute_type):
    model = whisperx.load_model("large-v3", device=device, compute_type=compute_type, download_root=MODELS, language="en")
    result = model.transcribe(audio, language="en", batch_size=batch_size)
    return result


def load_audio_file(audio_file: str):
    return whisperx.load_audio(audio_file)


def whisper_to_json(transcription):
    segments = []
    for segment in transcription["segments"]:
        start_timestamp = format_timestamp(segment["start"])
        end_timestamp = format_timestamp(segment["end"])
        segments.append({
            "speaker": segment.get("speaker") or "SPEAKER_NULL",
            "startTimestamp": start_timestamp.strip(),
            "endTimestamp": end_timestamp.strip(),
            "transcription": segment["text"].strip(),
            "startTime": segment["start"],
            "endTime": segment["end"],
            "type": "statement",
            "children": [{"text": segment["text"].strip()}],
            "words": segment["words"]
        })

    merged_segments = []
    for segment in segments:
        if merged_segments and merged_segments[-1]["speaker"] == segment["speaker"]:
            merged_segments[-1]["endTimestamp"] = segment["endTimestamp"]
            merged_segments[-1]["endTime"] = segment["endTime"]
            merged_segments[-1]["transcription"] += " " + segment["transcription"]
            merged_segments[-1]["children"].append({"text": segment["transcription"]})
            merged_segments[-1]["words"].extend(segment["words"])
        else:
            merged_segments.append(segment)

    return (segments, merged_segments)
