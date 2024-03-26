# `@auohp/scripts`
![](mascot.gif)

## Preparing audio
Using the least-compressed video file available, extract an audio track as FLAC and upload it to S3. Since this is an internal API, it's not useful to optimize asset size; given that crisp speech in well-separated voices helps the transcriber, it's better to optimize for quality.

```bash
ffmpeg -i '035_larry_kramer.mp4' -vn -c:a flac -y 035_larry_kramer.flac
```

## Custom language model
The project includes a custom language model in Amazon Transcribe. [Trained on](./src/train-custom-model.fish) unstructured but human-verified transcriptions of the interviews, it more reliably catches acronyms, proper nouns, and terms of art.
