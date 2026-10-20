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
    /// The source track's rate and channel count before mixdown and resampling.
    /// A 16 kHz mono input skips both `mix_to_mono` and `resample`, so a decode
    /// of it validates neither.
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

    let prev_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let probed_res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        symphonia::default::get_probe().format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
    }));
    std::panic::set_hook(prev_hook);

    let probed = probed_res
        .map_err(|_| anyhow::anyhow!("corrupted audio container caused decoder panic"))?
        .context("unsupported audio format")?;

    let mut format = probed.format;

    // Pick the first track with a sample rate and a real codec: symphonia lists
    // every trak, video included, and the video track comes first in this corpus.
    let track = format
        .tracks()
        .iter()
        .find(|t| {
            t.codec_params.codec != symphonia::core::codecs::CODEC_TYPE_NULL
                && t.codec_params.sample_rate.is_some()
        })
        .ok_or_else(|| {
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
    anyhow::ensure!(
        declared_rate > 0,
        "audio track sample rate must be positive, got {declared_rate}"
    );

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .context("unsupported audio codec")?;

    // Take channel count and rate from the decoded frames, not `codec_params`:
    // symphonia cannot recover a channel map from the `esds` of the original
    // masters, so `codec_params.channels` is `None` for stereo files.
    let mut channels: Option<usize> = None;
    let mut decoded_rate: Option<u32> = None;

    // Frame count from the container, used only to size the buffer up front;
    // growing by doubling would hold old and new buffers at once on a
    // multi-hour master.
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

        // The first decoded frame is when the true layout is first known.
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

    let mono = if channels > 1 {
        mix_to_mono(raw_samples, channels)
    } else {
        raw_samples
    };

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
/// Avoids a second output-sized buffer, which matters on a multi-hour master.
/// The write index `i` always trails the read index `i * channels`, so the fold
/// never overwrites unread samples. `truncate` drops any trailing partial frame.
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

fn resample(
    samples: &[f32],
    from_rate: u32,
    to_rate: u32,
    cfg: &AudioConfig,
) -> Result<Vec<f32>> {
    const MAX_SUPPORTED_SAMPLE_RATE: u32 = 384_000;
    anyhow::ensure!(
        from_rate > 0 && from_rate <= MAX_SUPPORTED_SAMPLE_RATE,
        "input sample rate out of bounds (1..={MAX_SUPPORTED_SAMPLE_RATE}), got {from_rate}"
    );
    anyhow::ensure!(
        to_rate > 0 && to_rate <= MAX_SUPPORTED_SAMPLE_RATE,
        "target sample rate out of bounds (1..={MAX_SUPPORTED_SAMPLE_RATE}), got {to_rate}"
    );

    let ratio = to_rate as f64 / from_rate as f64;
    anyhow::ensure!(
        (1.0 / 64.0..=64.0).contains(&ratio),
        "resampling ratio out of bounds (0.015625..=64.0), got {ratio}"
    );

    // Must match the `chunk_size` given to `new_sinc` below.
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

    // `FixedAsync::Input` expects exactly `chunk` input samples per call and keeps
    // a `sinc_len / 2` history to stitch chunk boundaries; one-shot resampling of
    // a single huge chunk overreads.
    let mut resampler = Async::<f32>::new_sinc(
        ratio,
        2.0,
        &params,
        chunk,
        1, // mono
        FixedAsync::Input,
    )?;

    let expected = (samples.len() as u64 * to_rate as u64 / from_rate as u64) as usize;
    let capacity = expected.min(100 * 1024 * 1024); // max 100M floats initial capacity
    let mut output = Vec::with_capacity(capacity);

    // Feed chunk-sized slices, zero-padding the final partial chunk and reusing
    // buffers across chunks.
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

    // Zero-padding the tail can yield a few extra samples.
    output.truncate(expected.min(output.len()));

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pins stereo arithmetic against a hand-computed value.
    #[test]
    fn mix_to_mono_averages_stereo_frames() {
        let interleaved = vec![1.0, 3.0, -2.0, 2.0, 0.5, 0.5];
        assert_eq!(mix_to_mono(interleaved, 2), vec![2.0, 0.0, 0.5]);
    }

    /// A bad in-place fold reads already-overwritten samples; a strictly
    /// increasing signal makes that a visible discontinuity.
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

    /// Mono passes through untouched (the 16 kHz WAV control path).
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

    #[test]
    fn test_resample_out_of_bounds_rates_return_error() {
        let cfg = AudioConfig::default();
        let samples = vec![0.1f32; 100];
        assert!(resample(&samples, 384_001, WHISPER_SAMPLE_RATE, &cfg).is_err());
        assert!(resample(&samples, 48_000, 384_001, &cfg).is_err());
        assert!(resample(&samples, u32::MAX, WHISPER_SAMPLE_RATE, &cfg).is_err());
        assert!(resample(&samples, WHISPER_SAMPLE_RATE, u32::MAX, &cfg).is_err());
        // Extreme ratio check (1 Hz has ratio 16000 > 64)
        assert!(resample(&samples, 1, WHISPER_SAMPLE_RATE, &cfg).is_err());
        assert!(resample(&samples, WHISPER_SAMPLE_RATE, 1, &cfg).is_err());
    }
}
