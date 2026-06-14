//! Audio loading for the whisper.cpp comparison harness.
//!
//! Deliberately minimal: the input is assumed to ALREADY be a mono, 16 kHz
//! WAV --- the exact format whisper.cpp expects --- so that no decoding,
//! channel mixdown, or resampling sits between the file on disk and the model.
//!
//! This replaces the previous symphonia + rubato pipeline.  That chain (MP4
//! container decode --> stereo-to-mono averaging --> sinc resample to 16 kHz)
//! was suspected of corrupting the mel spectrogram and triggering whisper.cpp's
//! hallucination loop.  By reading pre-conformed audio verbatim we remove every
//! upstream variable: any remaining mistranscription must originate in the
//! model or its decode parameters, not in our preprocessing.
//!
//! The mono/16 kHz contract is *asserted*, not enforced by transformation: if
//! the file does not already match, we bail loudly rather than silently
//! resampling and reintroducing the very artifacts we are trying to rule out.
//! Convert inputs ahead of time, e.g.:
//!   ffmpeg -i clip.mp4 -ac 1 -ar 16000 -c:a pcm_s16le clip.wav

use anyhow::{Context, Result, bail};
use std::path::Path;

const WHISPER_SAMPLE_RATE: u32 = 16_000;

/// Decoded audio ready for Whisper: 16 kHz mono f32 samples in [-1.0, 1.0].
pub struct DecodedAudio {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
}

/// Load a mono, 16 kHz WAV file as f32 PCM.
///
/// Errors if the file is not mono or not 16 kHz --- the harness assumes the
/// caller has already conformed the audio (see module docs).
pub fn decode_file(path: &Path) -> Result<DecodedAudio> {
    let reader = hound::WavReader::open(path)
        .with_context(|| format!("failed to open {}", path.display()))?;

    let spec = reader.spec();

    // Assumption guards: refuse to proceed on a format mismatch rather than
    // silently mistranscribing.  These are the two invariants whisper.cpp
    // depends on and that we have promised not to fix up ourselves.
    if spec.channels != 1 {
        bail!(
            "expected mono audio, got {} channels --- reconform with `ffmpeg -ac 1`",
            spec.channels
        );
    }
    if spec.sample_rate != WHISPER_SAMPLE_RATE {
        bail!(
            "expected {WHISPER_SAMPLE_RATE} Hz, got {} Hz --- reconform with `ffmpeg -ar 16000`",
            spec.sample_rate
        );
    }

    let samples = read_samples(reader, spec)?;

    Ok(DecodedAudio {
        samples,
        sample_rate: WHISPER_SAMPLE_RATE,
    })
}

/// Read every sample from a (mono, 16 kHz) WAV reader into f32 PCM in the
/// range [-1.0, 1.0], the amplitude convention whisper.cpp expects.
fn read_samples(
    reader: hound::WavReader<std::io::BufReader<std::fs::File>>,
    spec: hound::WavSpec,
) -> Result<Vec<f32>> {
    // TODO(human): turn `reader` into a `Vec<f32>` in [-1.0, 1.0].
    //
    // hound exposes samples two ways, and which one is valid depends on
    // `spec.sample_format` / `spec.bits_per_sample`:
    //   - hound::SampleFormat::Int   --> reader.into_samples::<i32>() (covers
    //     8/16/24/32-bit integer PCM; values are sign-extended to i32)
    //   - hound::SampleFormat::Float --> reader.into_samples::<f32>() (already
    //     in [-1.0, 1.0]; pass through)
    //
    // For integer PCM you must normalise by the full-scale value for that bit
    // depth: divide by 2^(bits_per_sample - 1).  Each `.into_samples()` item is
    // a `Result<T, hound::Error>`, so decide how to surface a mid-stream read
    // error (`?` inside a `.map`, `collect::<Result<_, _>>()`, etc.).//
    let full_scale = 2.0_f32.powi((spec.bits_per_sample as i32) - 1);

    let samples = match spec.sample_format {
        hound::SampleFormat::Float => {
            let mut float_samples: Vec<f32> = vec![];

            for sample in reader.into_samples::<f32>() {
                if let Ok(raw_float) = sample {
                    float_samples.push(raw_float);
                };
            }

            float_samples
        }
        hound::SampleFormat::Int => {
            let mut float_samples: Vec<f32> = vec![];

            for sample in reader.into_samples::<i32>() {
                if let Ok(raw_int) = sample {
                    let normalized = raw_int as f32 / full_scale;
                    float_samples.push(normalized);
                };
            }

            float_samples
        }
    };

    Ok(samples)
}
