# `@auohp/scripts`
![](mascot.gif)

Example scripts for interacting with key components of AUOHP transcription infrastructure.

## Amazon Transcribe
[Amazon Transcribe](https://aws.amazon.com/transcribe/) is an Amazon Web
Services product that uses machine learning to create transcripts of audio
files.

[`aws-transcribe.fish`](./src/aws-transcribe.fish) sketches interactions with
the features we use. First, it trains [a custom language
model](https://docs.aws.amazon.com/transcribe/latest/dg/custom-language-models.html)
on the text of interviews transcribed by humans. A large corpus of accurate text
helps the model detect things like "GMHC" that ML models struggle with out of
the box.

```sh
./src/aws-transcribe.fish path/to/pdfs
```

Custom model in hand, it extracts audio from interview videos, uploads it to
Amazon S3, and kicks off transcription jobs. These will eventually deposit
transcripts back in S3, in three formats: WebVTT and SRT captions, and a
structured JSON record.

```sh
./src/aws-transcribe.fish path/to/interviews
```

Auto-generated captions usually need additional manicuring: AUOHP apps ignore
them in favor of the JSON documents. But it couldn't hurt to have a fallback.
