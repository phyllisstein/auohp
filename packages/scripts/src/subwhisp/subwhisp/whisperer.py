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


def transcribe_audio_file(audio_file: str, device: str = device, batch_size: int = 16, compute_type: str = compute_type):
    model = whisperx.load_model("large-v3-turbo", device=device, compute_type=compute_type, download_root=MODELS, language="en")
    audio = whisperx.load_audio(audio_file)
    result = model.transcribe(audio, language="en")

    model_a, metadata = whisperx.load_align_model(language_code="en", device=device, model_dir=MODELS, model_name="facebook/wav2vec2-base-960h")
    aligned_result = whisperx.align(result["segments"], model_a, metadata, audio, device, return_char_alignments=False)

    diarize_model = whisperx.DiarizationPipeline("pyannote/speaker-diarization-3.1", use_auth_token="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", device=device)
    diarize_segments = diarize_model(audio)

    diarized_result = whisperx.assign_word_speakers(diarize_segments, aligned_result)

    segments = []
    for segment in diarized_result["segments"]:
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

    return segments
