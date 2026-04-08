//! Audio decoding and resampling.
//!
//! Decodes audio from any format symphonia supports (MP4/AAC, WAV, etc.)
//! and resamples to 16 kHz mono f32---the format whisper.cpp expects.

use anyhow::{Context, Result};
use audioadapter_buffers::direct::SequentialSliceOfVecs;
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

const WHISPER_SAMPLE_RATE: u32 = 16_000;

/// Decoded audio ready for Whisper: 16 kHz mono f32 samples in [-1.0, 1.0].
pub struct DecodedAudio {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
}

/// Decode an audio/video file to 16 kHz mono f32.
pub fn decode_file(path: &std::path::Path) -> Result<DecodedAudio> {
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

    // Find the first audio track.
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != symphonia::core::codecs::CODEC_TYPE_NULL)
        .context("no audio track found")?;

    let track_id = track.id;
    let source_rate = track
        .codec_params
        .sample_rate
        .context("audio track has no sample rate")?;
    let channels = track.codec_params.channels.map(|c| c.count()).unwrap_or(1);

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .context("unsupported audio codec")?;

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

        let mut sample_buf = SampleBuffer::<f32>::new(duration as u64, spec);
        sample_buf.copy_interleaved_ref(decoded);
        raw_samples.extend_from_slice(sample_buf.samples());
    }

    // Mix down to mono if multi-channel.
    let mono = if channels > 1 {
        mix_to_mono(&raw_samples, channels)
    } else {
        raw_samples
    };

    // Resample to 16 kHz if needed.
    let samples = if source_rate != WHISPER_SAMPLE_RATE {
        resample(&mono, source_rate, WHISPER_SAMPLE_RATE)?
    } else {
        mono
    };

    Ok(DecodedAudio {
        samples,
        sample_rate: WHISPER_SAMPLE_RATE,
    })
}

fn mix_to_mono(interleaved: &[f32], channels: usize) -> Vec<f32> {
    let inv = 1.0 / channels as f32;
    interleaved
        .chunks_exact(channels)
        .map(|frame| frame.iter().sum::<f32>() * inv)
        .collect()
}

// FIXME: Extracting audio from an MP4 video file contributes to corrupted
// transcriptions. Passing in a WAV already in 16k `pcm_s16le` bypasses these
// transformations and yields higher-quality output from Whisper, but deviates
// from the app's video-centric use case.

// 4096-frame chunks --- large enough to amortise per-call overhead, small
// enough to fit comfortably in L1/L2 cache.  Must match the `chunk_size`
// argument passed to `new_sinc` below.
const RESAMPLE_CHUNK: usize = 4096;

fn resample(samples: &[f32], from_rate: u32, to_rate: u32) -> Result<Vec<f32>> {
    let params = SincInterpolationParameters {
        sinc_len: 256,
        f_cutoff: 0.95,
        interpolation: SincInterpolationType::Linear,
        oversampling_factor: 256,
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
        RESAMPLE_CHUNK,
        1, // mono
        FixedAsync::Input,
    )?;

    let expected = (samples.len() as f64 * to_rate as f64 / from_rate as f64).round() as usize;
    let mut output = Vec::with_capacity(expected + RESAMPLE_CHUNK);

    // Feed the audio in RESAMPLE_CHUNK-sized slices, zero-padding the final
    // partial chunk.  Rubato handles each boundary cleanly via its history buf.
    for chunk in samples.chunks(RESAMPLE_CHUNK) {
        let mut buf = chunk.to_vec();
        buf.resize(RESAMPLE_CHUNK, 0.0); // no-op for full chunks

        let input = vec![buf];
        let adapter = SequentialSliceOfVecs::new(&input, 1, RESAMPLE_CHUNK)
            .map_err(|e| anyhow::anyhow!("adapter error: {e}"))?;
        output.extend_from_slice(&resampler.process(&adapter, 0, None)?.take_data());
    }

    // Trim to the exact expected length; the last few chunks may produce a
    // handful of extra samples due to the zero-padded tail.
    output.truncate(expected.min(output.len()));

    Ok(output)
}
