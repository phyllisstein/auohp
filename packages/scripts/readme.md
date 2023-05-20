# `@auohp/scripts`
![](mascot.gif)

## Preparing audio
Extract the audio from each interview video with ffmpeg. Though the uncompressed
WAV audio files can be massive, ffmpeg's `-segment` flag can create nice little
chunks instead. The audio can remain relatively clear when uploads are split,
and the service can transcribe them in parallel.

For example, broken into four files, Larry Kramer's 1hr 43min interview keeps a
respectable sample rate, without any one segment cracking 100MB.

```bash
ffmpeg -i '035_larry_kramer.mp4' -vn -f segment -segment_time 1551 -ar 4800 -ac 1 -acodec pcm_s16le -y 035-%03d.wav
# => 035-000.wav -- 035-003.wav
```

Each file will kick off a parallel transcription job, returning results in a quarter of the time. Don't be afraid to fiddle with the "segment length" and "sample rate" dials---prefer more segments of good quality to fewer segments of poor quality.
