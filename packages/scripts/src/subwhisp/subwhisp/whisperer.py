import whisperx
import platform
import os


MODELS = os.path.join(os.environ["HOME"], ".subwhisp")


if platform.system() == "Darwin":
    device = "cpu"
    compute_type = "int8"
else:
    device = "cuda"
    compute_type = "float16"


def transcribe_audio_file(audio_file: str, device: str = device, batch_size: int = 16, compute_type: str = compute_type):
    model = whisperx.load_model("large-v3", device=device, compute_type=compute_type, download_root=MODELS)
    audio = whisperx.load_audio(audio_file)
    result = model.transcribe(audio, batch_size=batch_size)

    model_a, metadata = whisperx.load_align_model(language_code=result["language"], device=device, model_dir=MODELS, model_name="WAV2VEC2_ASR_BASE_960H")
    aligned_result = whisperx.align(result["segments"], model_a, metadata, audio, device, return_char_alignments=False)

    diarize_model = whisperx.DiarizationPipeline("pyannote/speaker-diarization-3.1", use_auth_token="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", device=device)
    diarize_segments = diarize_model(audio, min_speakers=1, max_speakers=3)

    diarized_result = whisperx.assign_word_speakers(diarize_segments, aligned_result)

    return diarized_result
