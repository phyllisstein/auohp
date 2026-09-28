//! Audio decoding and resampling.
//!
//! Decodes audio from any format symphonia supports (MP4/AAC, WAV, etc.)
//! and resamples to 16 kHz mono f32---the format whisper.cpp expects.

use anyhow::{Context, Result};
use audioadapter_buffers::direct::InterleavedSlice;
use rubato::{
    Async, FixedAsync, Resampler, SincInterpolationParameters, SincInterpolationType,
    WindowFunction,
};
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

use super::config::{AudioConfig, Interpolation};

const WHISPER_SAMPLE_RATE: u32 = 16_000;

/// Decoded audio ready for Whisper: 16 kHz mono f32 samples in [-1.0, 1.0].
#[derive(Debug, Clone, PartialEq)]
pub struct DecodedAudio {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    /// The source track's rate and channel count, before mixdown and resampling.
    /// Recorded so tests and manifests can tell whether this decode actually
    /// exercised the transform chain or bypassed it --- a 16 kHz mono input
    /// skips both `mix_to_mono` and `resample`, and therefore validates neither.
    pub source_sample_rate: u32,
    pub source_channels: usize,
}

/// Decode an audio/video file to 16 kHz mono f32 using default parameters.
pub fn decode_file(path: &std::path::Path) -> Result<DecodedAudio> {
    decode_file_with(path, &AudioConfig::default())
}

/// Decode an audio/video file to 16 kHz mono f32.
pub fn decode_file_with(path: &std::path::Path, cfg: &AudioConfig) -> Result<DecodedAudio> {
    let file =
        std::fs::File::open(path).with_context(|| format!("failed to open {}", path.display()))?;

    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .context("unsupported audio format")?;

    let mut format = probed.format;

    // Find the first *audio* track.
    //
    // Selecting on "codec is not NULL" is not the same test and was quietly
    // wrong: symphonia emits a Track for every trak in the container, video
    // included, and in this corpus the video track comes first. It happened to
    // work only because symphonia 0.5 is audio-only and files video sample
    // entries under `SampleEntry::Other` --- its `Video` variant is commented
    // out. One upstream release adding video support would have silently started
    // feeding picture data to Whisper.
    //
    // Sample rate and channel count are the properties we actually require
    // downstream, so requiring them here makes the selection self-evidently
    // correct rather than incidentally correct.
    let track = format
        .tracks()
        .iter()
        .find(|t| {
            t.codec_params.codec != symphonia::core::codecs::CODEC_TYPE_NULL
                && t.codec_params.sample_rate.is_some()
        })
        .ok_or_else(|| {
            // Say what was actually there. "No audio track found" on a file that
            // plainly has one sends you looking in the wrong place; the track
            // list distinguishes "container not parsed" from "track present but
            // missing the parameters we require".
            let seen: Vec<String> = format
                .tracks()
                .iter()
                .map(|t| {
                    format!(
                        "#{} codec={:?} rate={:?} channels={:?}",
                        t.id,
                        t.codec_params.codec,
                        t.codec_params.sample_rate,
                        t.codec_params.channels.map(|c| c.count())
                    )
                })
                .collect();
            anyhow::anyhow!("no audio track found; tracks present: [{}]", seen.join("; "))
        })?;

    let track_id = track.id;
    let declared_rate = track
        .codec_params
        .sample_rate
        .context("audio track has no sample rate")?;

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .context("unsupported audio codec")?;

    // Channel count and rate are taken from the decoded frames, not from
    // `codec_params`.
    //
    // This is not defensiveness --- it is the fix for a real, silent corruption.
    // The original masters in this corpus carry an `esds` from which symphonia
    // cannot recover a channel map, so `codec_params.channels` is `None` even
    // though the audio is stereo. The previous code read that as
    // `.unwrap_or(1)`, so `mix_to_mono` never ran and an interleaved L,R,L,R
    // stream was handed to the resampler as if it were one mono channel: double
    // length, both channels smeared together, every timestamp wrong. It failed
    // silently and only on the files that matter, which is why it survived as
    // folklore about "MP4 extraction" rather than being found.
    //
    // The decoder always knows the true layout, because it just produced the
    // frames. Trust it over the container metadata.
    let mut channels: Option<usize> = None;
    let mut decoded_rate: Option<u32> = None;

    // Frame count from the container, used only to size the buffer up front.
    // Growing by doubling is fine for a 34-minute clip and fatal for a full
    // interview: the last realloc on a 3.4-hour stereo master holds the old 4 GB
    // and the new 8 GB live at once, which is more than this box has.
    let declared_frames = track.codec_params.n_frames;

    let mut raw_samples: Vec<f32> = Vec::new();

    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(symphonia::core::errors::Error::IoError(ref e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(e) => return Err(e.into()),
        };

        if packet.track_id() != track_id {
            continue;
        }

        let decoded = decoder.decode(&packet)?;
        let spec = *decoded.spec();
        let duration = decoded.capacity();

        // `channels.is_none()` is exactly "this is the first decoded frame", which
        // is the first moment the true layout is known --- see above for why the
        // container's channel count cannot be trusted for it.
        if channels.is_none()
            && let Some(frames) = declared_frames
        {
            raw_samples.reserve_exact(frames as usize * spec.channels.count());
        }
        channels.get_or_insert(spec.channels.count());
        decoded_rate.get_or_insert(spec.rate);

        let mut sample_buf = SampleBuffer::<f32>::new(duration as u64, spec);
        sample_buf.copy_interleaved_ref(decoded);
        raw_samples.extend_from_slice(sample_buf.samples());
    }

    let channels = channels.context("audio track decoded no frames")?;
    let source_rate = decoded_rate.unwrap_or(declared_rate);

    // Mix down to mono if multi-channel.
    let mono = if channels > 1 {
        mix_to_mono(raw_samples, channels)
    } else {
        raw_samples
    };

    // Resample to 16 kHz if needed.
    let samples = if source_rate != WHISPER_SAMPLE_RATE {
        resample(&mono, source_rate, WHISPER_SAMPLE_RATE, cfg)?
    } else {
        mono
    };

    Ok(DecodedAudio {
        samples,
        sample_rate: WHISPER_SAMPLE_RATE,
        source_sample_rate: source_rate,
        source_channels: channels,
    })
}

/// Fold interleaved frames down to mono, **in place**.
///
/// The iterator version --- `chunks_exact(channels).map(...).collect()` --- reads
/// better and allocates a second buffer the size of the output. That is free on a
/// clip and not free on a full interview, where the input is 4 GB and the mono
/// result is another 2 GB held live beside it.
///
/// The fold can overwrite its own input because the write index `i` always trails
/// the read index `i * channels` whenever `channels >= 1`: by the time frame `i` is
/// written, everything at or beyond `i * channels` is still untouched. So this
/// takes the `Vec` by value, folds forwards, and truncates. `truncate` also
/// discards any trailing partial frame, which is what `chunks_exact` did.
fn mix_to_mono(mut interleaved: Vec<f32>, channels: usize) -> Vec<f32> {
    let inv = 1.0 / channels as f32;
    let frames = interleaved.len() / channels;
    for i in 0..frames {
        let base = i * channels;
        let sum: f32 = interleaved[base..base + channels].iter().sum();
        interleaved[i] = sum * inv;
    }
    interleaved.truncate(frames);
    interleaved
}

// RESOLVED: MP4 extraction did corrupt transcriptions, but the resampler was
// never the culprit.
//
// The cause was the channel count. Symphonia cannot recover a channel map from
// the `esds` of this corpus's original masters, so `codec_params.channels` is
// `None` and the previous `.unwrap_or(1)` read a stereo file as mono: mix-down
// was skipped and an interleaved L,R,L,R stream went into the resampler as if it
// were a single channel. See `decode_file_with`, which now takes the layout from
// the decoded frames instead.
//
// `tests/decode.rs` measures the rest, with Whisper out of the loop. Both the
// MP4 and the WAV paths decode the same AAC, so the comparison isolates *this
// resampler* against the external tool that produced the WAV --- not "MP4
// handling" in the abstract. It scores 0.998 normalised cross-correlation with
// zero residual drift across the file, so the sinc configuration below is
// sound and is not a useful thing to tune.

fn resample(
    samples: &[f32],
    from_rate: u32,
    to_rate: u32,
    cfg: &AudioConfig,
) -> Result<Vec<f32>> {
    anyhow::ensure!(
        from_rate > 0 && to_rate > 0,
        "sample rates must be positive, got from_rate={from_rate}, to_rate={to_rate}"
    );

    // Chunk size must match the `chunk_size` given to `new_sinc` below.
    // 4096 frames by default --- large enough to amortise per-call overhead,
    // small enough to sit comfortably in L1/L2 cache.
    let chunk = cfg.resample_chunk;

    let params = SincInterpolationParameters {
        sinc_len: cfg.sinc_len,
        f_cutoff: cfg.f_cutoff,
        interpolation: match cfg.interpolation {
            Interpolation::Nearest => SincInterpolationType::Nearest,
            Interpolation::Linear => SincInterpolationType::Linear,
            Interpolation::Quadratic => SincInterpolationType::Quadratic,
            Interpolation::Cubic => SincInterpolationType::Cubic,
        },
        oversampling_factor: cfg.oversampling_factor,
        window: WindowFunction::BlackmanHarris2,
    };

    // `Async` with `FixedAsync::Input` expects exactly `chunk_size` input
    // samples per call.  It keeps a `sinc_len / 2` sample history internally
    // (the left half of the FIR kernel) so it can correctly stitch boundaries
    // between chunks --- which is exactly what lets us avoid the one-shot
    // overread panic.  In streaming mode the history is populated from prior
    // real audio; in one-shot mode with a single huge chunk, it's zeros and
    // the right-half lookahead has nowhere to read from.
    let mut resampler = Async::<f32>::new_sinc(
        to_rate as f64 / from_rate as f64,
        2.0,
        &params,
        chunk,
        1, // mono
        FixedAsync::Input,
    )?;

    let expected = (samples.len() as f64 * to_rate as f64 / from_rate as f64).round() as usize;
    let mut output = Vec::with_capacity(expected + chunk);

    // Feed the audio in chunk-sized slices, zero-padding the final partial
    // chunk. Rubato handles each boundary cleanly via its history buf.
    //
    // Reusable buffers eliminate per-chunk heap allocations during resampling.
    let mut tail_buf = vec![0.0f32; chunk];
    let max_out = resampler.output_frames_max();
    let mut out_chunk = vec![0.0f32; max_out];

    for block in samples.chunks(chunk) {
        let in_adapter = if block.len() == chunk {
            InterleavedSlice::new(block, 1, chunk)
                .map_err(|e| anyhow::anyhow!("input adapter error: {e}"))?
        } else {
            tail_buf[..block.len()].copy_from_slice(block);
            tail_buf[block.len()..].fill(0.0);
            InterleavedSlice::new(&tail_buf, 1, chunk)
                .map_err(|e| anyhow::anyhow!("input adapter error: {e}"))?
        };

        let needed_out = resampler.output_frames_next();
        let mut out_adapter = InterleavedSlice::new_mut(&mut out_chunk, 1, needed_out)
            .map_err(|e| anyhow::anyhow!("output adapter error: {e}"))?;

        let (_in_frames, out_frames) =
            resampler.process_into_buffer(&in_adapter, &mut out_adapter, None)?;
        output.extend_from_slice(&out_chunk[..out_frames]);
    }

    // Trim to the exact expected length; the last few chunks may produce a
    // handful of extra samples due to the zero-padded tail.
    output.truncate(expected.min(output.len()));

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The in-place fold is only safe because the write index trails the read
    /// index. Stereo is the case that actually ships, so pin its arithmetic
    /// against a value computed by hand rather than by the same expression.
    #[test]
    fn mix_to_mono_averages_stereo_frames() {
        let interleaved = vec![1.0, 3.0, -2.0, 2.0, 0.5, 0.5];
        assert_eq!(mix_to_mono(interleaved, 2), vec![2.0, 0.0, 0.5]);
    }

    /// The failure mode of a bad in-place fold is that later frames read samples
    /// the fold has already overwritten, so it only shows up once the write index
    /// has had room to catch up. A long run with a strictly increasing signal
    /// makes any such clobber a visible discontinuity rather than a plausible
    /// number.
    #[test]
    fn mix_to_mono_does_not_clobber_its_own_input() {
        let frames = 4096;
        let interleaved: Vec<f32> = (0..frames * 2).map(|i| i as f32).collect();
        let mono = mix_to_mono(interleaved, 2);

        assert_eq!(mono.len(), frames);
        for (i, got) in mono.iter().enumerate() {
            let want = (4 * i + 1) as f32 / 2.0; // mean of 2i and 2i+1
            assert_eq!(*got, want, "frame {i} was folded from clobbered samples");
        }
    }

    /// Mono input must pass through untouched --- this is the branch the 16 kHz
    /// WAV control takes, and if it were to average anything the control would
    /// stop being one.
    #[test]
    fn mix_to_mono_is_identity_for_one_channel() {
        let samples = vec![0.25, -0.5, 1.0];
        assert_eq!(mix_to_mono(samples.clone(), 1), samples);
    }

    /// `chunks_exact` dropped a trailing partial frame; `truncate` must too.
    #[test]
    fn mix_to_mono_drops_a_trailing_partial_frame() {
        let interleaved = vec![1.0, 3.0, 5.0];
        assert_eq!(mix_to_mono(interleaved, 2), vec![2.0]);
    }

    #[test]
    fn resample_empty_input_returns_empty() {
        let cfg = AudioConfig::default();
        let res = resample(&[], 44_100, WHISPER_SAMPLE_RATE, &cfg).unwrap();
        assert!(res.is_empty());
    }

    #[test]
    fn resample_preserves_sine_frequency_and_length() {
        let cfg = AudioConfig::default();
        let from_rate = 44_100;
        let to_rate = WHISPER_SAMPLE_RATE;
        let duration_s = 0.5;
        let freq = 440.0;
        let n_samples = (from_rate as f64 * duration_s) as usize;
        let samples: Vec<f32> = (0..n_samples)
            .map(|i| (2.0 * std::f32::consts::PI * freq * (i as f32) / from_rate as f32).sin())
            .collect();

        let resampled = resample(&samples, from_rate, to_rate, &cfg).unwrap();
        let expected_len = (samples.len() as f64 * to_rate as f64 / from_rate as f64).round() as usize;
        assert_eq!(resampled.len(), expected_len);

        let max_val = resampled.iter().fold(0.0f32, |acc, &x| acc.max(x.abs()));
        assert!(max_val > 0.8 && max_val <= 1.05, "amplitude preserved: {max_val}");
    }

    #[test]
    fn resample_handles_arbitrary_and_partial_chunk_lengths() {
        let cfg = AudioConfig {
            resample_chunk: 1024,
            ..AudioConfig::default()
        };

        // Smaller than 1 chunk (e.g. 500 samples)
        let short_audio = vec![0.1f32; 500];
        let out_short = resample(&short_audio, 48_000, WHISPER_SAMPLE_RATE, &cfg).unwrap();
        let expected_short = (500.0f64 * 16_000.0 / 48_000.0).round() as usize;
        assert!((out_short.len() as i64 - expected_short as i64).abs() <= 2);

        // Exactly 1 chunk (1024 samples)
        let exact_audio = vec![0.1f32; 1024];
        let out_exact = resample(&exact_audio, 48_000, WHISPER_SAMPLE_RATE, &cfg).unwrap();
        let expected_exact = (1024.0f64 * 16_000.0 / 48_000.0).round() as usize;
        assert!((out_exact.len() as i64 - expected_exact as i64).abs() <= 2);

        // 1 chunk + 1 sample (1025 samples)
        let split_audio = vec![0.1f32; 1025];
        let out_split = resample(&split_audio, 48_000, WHISPER_SAMPLE_RATE, &cfg).unwrap();
        let expected_split = (1025.0f64 * 16_000.0 / 48_000.0).round() as usize;
        assert!((out_split.len() as i64 - expected_split as i64).abs() <= 2);
    }

    #[test]
    fn test_resample_zero_rates_return_error() {
        let cfg = AudioConfig::default();
        let samples = vec![0.1f32; 100];
        assert!(resample(&samples, 0, WHISPER_SAMPLE_RATE, &cfg).is_err());
        assert!(resample(&samples, 48_000, 0, &cfg).is_err());
        assert!(resample(&samples, 0, 0, &cfg).is_err());
    }
}
