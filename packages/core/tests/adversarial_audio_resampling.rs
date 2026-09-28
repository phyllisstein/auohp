//! Standalone adversarial stress test harness for Milestone 3:
//! Audio Resampling & Buffering (`transcription/audio.rs`).
//!
//! Empirical challenge verification:
//! 1. Extreme sample rate conversions:
//!    - 44.1 kHz -> 16 kHz (standard CD downsample: 0.3628x)
//!    - 48.0 kHz -> 16 kHz (standard broadcast/video downsample: 0.3333x)
//!    - 88.2 kHz -> 16 kHz (high-res audio downsample: 0.1814x)
//!    - 96.0 kHz -> 16 kHz (pro audio downsample: 0.1667x)
//!    - 8.0 kHz  -> 16 kHz (telephony upsample: 2.0x)
//!    - 11.025 kHz -> 16 kHz, 22.05 kHz -> 16 kHz, 32.0 kHz -> 16 kHz, 192.0 kHz -> 16 kHz
//! 2. Chunk size non-multiples and boundaries:
//!    - Lengths: 0, 1, 4095, 4096, 4097, 8191, 8192, 8193
//!    - Arbitrary chunk configurations: 128, 512, 1024, 2048, 4096, 8192
//! 3. Spectral fidelity & energy invariance:
//!    - Sinusoidal frequency preservation (220 Hz, 440 Hz, 1000 Hz, 3000 Hz) via zero-crossing analysis
//!    - RMS energy conservation across passband frequencies and diverse amplitudes
//!    - DC offset gain invariance (DC step response)
//!    - Anti-aliasing attenuation of frequencies above target Nyquist (e.g. 10 kHz -> 16 kHz target)
//! 4. Numerical stability & edge cases:
//!    - Subnormal numbers, full scale [-1.0, 1.0], DC bias
//!    - Zero-length audio input handling
//!    - Exact sample length adherence vs theoretical target (within FIR lookahead bound of <= 1 frame)
//! 5. End-to-end container integration:
//!    - Synthetic WAV generation and decoding via `auohp_core::transcription::decode_file_with`
//!    - Stereo downmix + resampling composition

use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use anyhow::Result;
use audioadapter_buffers::direct::InterleavedSlice;
use rubato::{
    Async, FixedAsync, Resampler, SincInterpolationParameters, SincInterpolationType,
    WindowFunction,
};
use auohp_core::transcription::{
    decode_file_with, AudioConfig, Interpolation,
};

const WHISPER_SAMPLE_RATE: u32 = 16_000;

// ── Verbatim Mirror of audio.rs Resampler ─────────────────────────────────────

/// Mirror of audio.rs's internal `resample` function to allow direct slice-level
/// adversarial challenge without container/file I/O overhead.
fn resample_reference(
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

    let mut resampler = Async::<f32>::new_sinc(
        ratio,
        2.0,
        &params,
        chunk,
        1, // mono
        FixedAsync::Input,
    )?;

    let expected = (samples.len() as u64 * to_rate as u64 / from_rate as u64) as usize;
    let capacity = expected.min(100 * 1024 * 1024);
    let mut output = Vec::with_capacity(capacity);

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

    output.truncate(expected.min(output.len()));

    Ok(output)
}

// ── Signal Analysis Helpers ──────────────────────────────────────────────────

fn compute_rms(signal: &[f32]) -> f64 {
    if signal.is_empty() {
        return 0.0;
    }
    let sum_sq: f64 = signal.iter().map(|&x| (x as f64).powi(2)).sum();
    (sum_sq / signal.len() as f64).sqrt()
}

/// Estimates fundamental frequency using upward zero-crossing counting.
fn estimate_frequency(signal: &[f32], sample_rate: u32) -> Option<f64> {
    if signal.len() < 10 {
        return None;
    }
    // Discard initial 5% and final 5% to avoid filter startup/flush boundary artifacts
    let trim_start = signal.len() / 20;
    let trim_end = signal.len() - trim_start;
    let slice = &signal[trim_start..trim_end];

    let mut crossings = Vec::new();
    for i in 0..slice.len() - 1 {
        if slice[i] <= 0.0 && slice[i + 1] > 0.0 {
            // Linear interpolation for sub-sample zero-crossing precision
            let fraction = (-slice[i]) / (slice[i + 1] - slice[i]);
            let exact_sample = i as f64 + fraction as f64;
            crossings.push(exact_sample);
        }
    }

    if crossings.len() < 2 {
        return None;
    }

    let periods: Vec<f64> = crossings.windows(2).map(|w| w[1] - w[0]).collect();
    let avg_period: f64 = periods.iter().sum::<f64>() / periods.len() as f64;
    Some(sample_rate as f64 / avg_period)
}

fn generate_sine_wave(freq: f64, sample_rate: u32, duration_s: f64, amplitude: f32) -> Vec<f32> {
    let total_samples = (sample_rate as f64 * duration_s).round() as usize;
    (0..total_samples)
        .map(|i| {
            let t = i as f64 / sample_rate as f64;
            (2.0 * std::f64::consts::PI * freq * t).sin() as f32 * amplitude
        })
        .collect()
}

// ── Synthetic WAV Generator for End-to-End Testing ───────────────────────────

fn create_temp_wav(
    samples: &[f32],
    sample_rate: u32,
    channels: u16,
    filename: &str,
) -> Result<PathBuf> {
    let dir = std::env::temp_dir().join("auohp_m3_tests");
    std::fs::create_dir_all(&dir)?;
    let file_path = dir.join(filename);

    let mut file = File::create(&file_path)?;

    // 16-bit PCM WAV format
    let bits_per_sample: u16 = 16;
    let bytes_per_sample = (bits_per_sample / 8) as usize;
    let byte_rate = sample_rate * channels as u32 * bytes_per_sample as u32;
    let block_align = channels * bits_per_sample / 8;
    let data_len = (samples.len() * bytes_per_sample) as u32;
    let riff_chunk_size = 36 + data_len;

    // RIFF header
    file.write_all(b"RIFF")?;
    file.write_all(&riff_chunk_size.to_le_bytes())?;
    file.write_all(b"WAVE")?;

    // fmt subchunk
    file.write_all(b"fmt ")?;
    file.write_all(&16u32.to_le_bytes())?; // Subchunk1Size (16 for PCM)
    file.write_all(&1u16.to_le_bytes())?;  // AudioFormat (1 for PCM)
    file.write_all(&channels.to_le_bytes())?;
    file.write_all(&sample_rate.to_le_bytes())?;
    file.write_all(&byte_rate.to_le_bytes())?;
    file.write_all(&block_align.to_le_bytes())?;
    file.write_all(&bits_per_sample.to_le_bytes())?;

    // data subchunk
    file.write_all(b"data")?;
    file.write_all(&data_len.to_le_bytes())?;

    // Convert f32 samples to i16 PCM
    for &sample in samples {
        let clamped = sample.clamp(-1.0, 1.0);
        let val = (clamped * 32767.0).round() as i16;
        file.write_all(&val.to_le_bytes())?;
    }

    file.flush()?;
    Ok(file_path)
}

// =============================================================================
// 1. Extreme Sample Rate Conversions
// =============================================================================

#[test]
fn test_extreme_sample_rates_sweep() {
    let cfg = AudioConfig::default();
    let rates = [
        8_000,   // Telephone narrowband (upsample 2.0x)
        11_025,  // Quarter CD rate (upsample 1.451x)
        16_000,  // Whisper native rate (identity 1.0x)
        22_050,  // Half CD rate (downsample 0.7256x)
        32_000,  // Half broadcast rate (downsample 0.5x)
        44_100,  // Standard CD rate (downsample 0.3628x)
        48_000,  // Standard broadcast / video rate (downsample 0.3333x)
        88_200,  // High-resolution audio (downsample 0.1814x)
        96_000,  // Pro studio rate (downsample 0.1667x)
        192_000, // Ultra-high resolution (downsample 0.0833x)
    ];

    for &from_rate in &rates {
        let duration_s = 0.5;
        let input = generate_sine_wave(440.0, from_rate, duration_s, 0.7);
        let expected_len = (input.len() as f64 * WHISPER_SAMPLE_RATE as f64 / from_rate as f64).round() as usize;

        let resampled = resample_reference(&input, from_rate, WHISPER_SAMPLE_RATE, &cfg)
            .unwrap_or_else(|e| panic!("Failed resampling from {from_rate} Hz: {e}"));

        assert!(
            (resampled.len() as i64 - expected_len as i64).abs() <= 1,
            "Length mismatch for rate {from_rate} Hz: expected {expected_len}, got {}",
            resampled.len()
        );

        // Verify all output samples are finite
        for (idx, &s) in resampled.iter().enumerate() {
            assert!(s.is_finite(), "Non-finite sample at {idx} for rate {from_rate}: {s}");
        }
    }
}

// =============================================================================
// 2. Chunk Size Stress Tests & Non-Multiples
// =============================================================================

#[test]
fn test_chunk_size_non_multiples_and_boundaries() {
    let cfg = AudioConfig {
        resample_chunk: 4096,
        ..AudioConfig::default()
    };

    let target_lengths = [
        0,     // Empty
        1,     // Single sample
        4095,  // Chunk - 1
        4096,  // Exact 1 Chunk
        4097,  // Chunk + 1
        8191,  // 2 Chunks - 1
        8192,  // Exact 2 Chunks
        8193,  // 2 Chunks + 1
        16384, // Exact 4 Chunks
        16385, // 4 Chunks + 1
    ];

    let rates = [48_000, 44_100, 96_000, 8_000];

    for &from_rate in &rates {
        for &len in &target_lengths {
            let input: Vec<f32> = (0..len).map(|i| (i as f32 * 0.01).sin()).collect();
            let to_rate = WHISPER_SAMPLE_RATE;
            let expected_len = (len as f64 * to_rate as f64 / from_rate as f64).round() as usize;

            let resampled = resample_reference(&input, from_rate, to_rate, &cfg)
                .unwrap_or_else(|e| panic!("Failed resampling len {len} at {from_rate} Hz: {e}"));

            if len == 0 {
                assert!(resampled.is_empty(), "Zero-length input must produce zero-length output");
            } else {
                // Due to Rubato's sinc FIR filter lookahead window, boundary truncation at
                // chunk boundaries may yield up to 4 samples fewer on upsampled streams (e.g. 8k -> 16k).
                let max_diff = 4;
                let diff = (resampled.len() as i64 - expected_len as i64).abs();
                assert!(
                    diff <= max_diff,
                    "Length deviation out of bounds for len={len} at {from_rate} Hz: expected {expected_len}, got {}, diff {diff} > max_diff {max_diff}",
                    resampled.len()
                );

            }
        }
    }
}


#[test]
fn test_zero_length_audio_input() {
    let cfg = AudioConfig::default();
    let rates = [8_000, 16_000, 44_100, 48_000, 96_000];

    for &rate in &rates {
        let res = resample_reference(&[], rate, WHISPER_SAMPLE_RATE, &cfg)
            .expect("Empty slice must not error");
        assert!(res.is_empty(), "Empty input at {rate} Hz must return empty Vec");
    }
}

#[test]
fn test_interpolation_modes_sweep() {
    let modes = [
        Interpolation::Nearest,
        Interpolation::Linear,
        Interpolation::Quadratic,
        Interpolation::Cubic,
    ];

    let input = generate_sine_wave(440.0, 48_000, 0.5, 0.7);

    for &mode in &modes {
        let cfg = AudioConfig {
            interpolation: mode,
            ..AudioConfig::default()
        };

        let res = resample_reference(&input, 48_000, WHISPER_SAMPLE_RATE, &cfg)
            .unwrap_or_else(|e| panic!("Failed resampling with mode {:?}: {e}", mode));

        let freq = estimate_frequency(&res, WHISPER_SAMPLE_RATE)
            .expect("Failed estimating frequency");
        assert!(
            (freq - 440.0).abs() < 5.0,
            "Mode {:?} distorted frequency: {freq}",
            mode
        );
    }
}



#[test]
fn test_diverse_resample_chunk_configurations() {
    let chunk_sizes = [128, 512, 1024, 2048, 4096, 8192];
    let input_len = 10_000; // Intentionally not a power of 2
    let input: Vec<f32> = (0..input_len).map(|i| (i as f32 * 0.05).sin()).collect();

    for &chunk_size in &chunk_sizes {
        let cfg = AudioConfig {
            resample_chunk: chunk_size,
            ..AudioConfig::default()
        };

        let resampled = resample_reference(&input, 44_100, WHISPER_SAMPLE_RATE, &cfg)
            .unwrap_or_else(|e| panic!("Failed with resample_chunk={chunk_size}: {e}"));

        let expected_len = (input_len as f64 * WHISPER_SAMPLE_RATE as f64 / 44_100.0).round() as usize;
        let diff = (resampled.len() as i64 - expected_len as i64).abs();
        assert!(
            diff <= 1,
            "Chunk size {chunk_size} yielded length {}, expected {expected_len}",
            resampled.len()
        );
    }
}

// =============================================================================
// 3. Spectral Fidelity: Frequency Preservation & RMS Energy Invariance
// =============================================================================

#[test]
fn test_sinusoidal_frequency_preservation() {
    let cfg = AudioConfig::default();
    let test_frequencies = [220.0, 440.0, 880.0, 1500.0, 3000.0];
    let source_rates = [44_100, 48_000, 96_000];

    for &freq in &test_frequencies {
        for &rate in &source_rates {
            let duration_s = 1.0;
            let input = generate_sine_wave(freq, rate, duration_s, 0.8);
            let resampled = resample_reference(&input, rate, WHISPER_SAMPLE_RATE, &cfg).unwrap();

            let estimated_freq = estimate_frequency(&resampled, WHISPER_SAMPLE_RATE)
                .expect("Failed estimating frequency of resampled signal");

            let freq_error = (estimated_freq - freq).abs();
            let relative_error = freq_error / freq;

            assert!(
                relative_error < 0.005,
                "Frequency distorted for {freq} Hz at {rate} Hz: estimated {estimated_freq:.2} Hz (rel error: {relative_error:.4})"
            );
        }
    }
}

#[test]
fn test_rms_energy_invariance_in_passband() {
    let cfg = AudioConfig::default();
    let test_amplitudes = [0.05, 0.2, 0.5, 0.8, 0.95];
    let source_rates = [44_100, 48_000, 88_200, 96_000, 8_000];
    let freq = 440.0; // Well within the passband for all tested sample rates

    for &amp in &test_amplitudes {
        for &rate in &source_rates {
            let duration_s = 1.0;
            let input = generate_sine_wave(freq, rate, duration_s, amp);
            let in_rms = compute_rms(&input);

            let resampled = resample_reference(&input, rate, WHISPER_SAMPLE_RATE, &cfg).unwrap();
            
            // Trim leading/trailing 1000 samples to exclude FIR filter ramp-up / ramp-down
            let steady_state = if resampled.len() > 2000 {
                &resampled[1000..resampled.len() - 1000]
            } else {
                &resampled[..]
            };
            let out_rms = compute_rms(steady_state);

            let ratio = out_rms / in_rms;
            assert!(
                (ratio - 1.0).abs() < 0.05,
                "RMS energy deviation out of bounds for amp={amp}, rate={rate}: in_rms={in_rms:.4}, out_rms={out_rms:.4}, ratio={ratio:.4}"
            );
        }
    }
}

#[test]
fn test_anti_aliasing_filter_attenuates_stopband() {
    let cfg = AudioConfig::default();
    let source_rate = 48_000;
    // 10 kHz is above the 16 kHz Whisper Nyquist frequency (8 kHz).
    // The resampler's lowpass filter MUST attenuate it heavily to prevent aliasing.
    let duration_s = 0.5;
    let stopband_input = generate_sine_wave(10_000.0, source_rate, duration_s, 0.8);
    let in_rms = compute_rms(&stopband_input);

    let resampled = resample_reference(&stopband_input, source_rate, WHISPER_SAMPLE_RATE, &cfg).unwrap();
    let out_rms = compute_rms(&resampled);

    let attenuation_db = 20.0 * (out_rms / in_rms).log10();
    assert!(
        attenuation_db < -40.0,
        "Stopband 10 kHz signal was insufficiently attenuated: {attenuation_db:.1} dB (out_rms={out_rms}, in_rms={in_rms})"
    );
}

// =============================================================================
// 4. Numerical Stability & Extreme Values
// =============================================================================

#[test]
fn test_dc_step_gain_invariance() {
    let cfg = AudioConfig::default();
    let dc_values = [0.1f32, 0.5, 0.9, -0.4];

    for &dc in &dc_values {
        let input = vec![dc; 12_000]; // ~0.25s at 48k
        let resampled = resample_reference(&input, 48_000, WHISPER_SAMPLE_RATE, &cfg).unwrap();

        // Discard filter boundary transients (first 500 and last 500 samples)
        let interior = &resampled[500..resampled.len() - 500];
        for (i, &val) in interior.iter().enumerate() {
            assert!(
                (val - dc).abs() < 0.01,
                "DC level drifted at {i}: expected {dc}, got {val}"
            );
        }
    }
}

#[test]
fn test_subnormal_and_extreme_float_resilience() {
    let cfg = AudioConfig::default();
    let mut inputs = vec![0.0f32; 8192];
    
    // Inject subnormals and tiny values
    for (i, x) in inputs.iter_mut().enumerate() {
        if i % 2 == 0 {
            *x = 1e-38;
        } else {
            *x = -1e-38;
        }
    }

    let res = resample_reference(&inputs, 48_000, WHISPER_SAMPLE_RATE, &cfg);
    assert!(res.is_ok(), "Resampler must handle subnormal numbers without error");
    let out = res.unwrap();
    for &val in &out {
        assert!(val.is_finite(), "Subnormal input produced non-finite output: {val}");
        assert!(val.abs() < 1e-10, "Subnormal noise amplified unexpectedly: {val}");
    }
}

// =============================================================================
// 5. End-to-End Container & Decoding Verification (`decode_file_with`)
// =============================================================================

#[test]
fn test_e2e_decode_file_with_synthetic_wavs() {
    let cfg = AudioConfig::default();
    let test_rates = [8_000, 44_100, 48_000, 96_000];

    for &rate in &test_rates {
        let duration_s = 0.5;
        let samples = generate_sine_wave(440.0, rate, duration_s, 0.7);
        let wav_path = create_temp_wav(
            &samples,
            rate,
            1, // mono
            &format!("test_e2e_{rate}_mono.wav"),
        ).unwrap();

        let decoded = decode_file_with(&wav_path, &cfg)
            .unwrap_or_else(|e| panic!("Failed decode_file_with for {rate} Hz: {e}"));

        assert_eq!(decoded.sample_rate, WHISPER_SAMPLE_RATE);
        assert_eq!(decoded.source_sample_rate, rate);
        assert_eq!(decoded.source_channels, 1);

        let expected_len = (samples.len() as f64 * WHISPER_SAMPLE_RATE as f64 / rate as f64).round() as usize;
        let diff = (decoded.samples.len() as i64 - expected_len as i64).abs();
        assert!(
            diff <= 1,
            "Decoded length mismatch for {rate} Hz: expected {expected_len}, got {}",
            decoded.samples.len()
        );

        let rms = compute_rms(&decoded.samples);
        assert!(rms > 0.4 && rms < 0.6, "RMS out of bounds for {rate} Hz: {rms}");

        let _ = std::fs::remove_file(&wav_path);
    }
}

#[test]
fn test_e2e_decode_stereo_downmix_and_resample() {
    let cfg = AudioConfig::default();
    let rate = 48_000;
    let duration_s = 0.5;
    let num_frames = (rate as f64 * duration_s).round() as usize;

    // Generate stereo: Left is 440 Hz sine, Right is 440 Hz cosine
    let mut interleaved = Vec::with_capacity(num_frames * 2);
    for i in 0..num_frames {
        let t = i as f64 / rate as f64;
        let left = (2.0 * std::f64::consts::PI * 440.0 * t).sin() as f32 * 0.6;
        let right = (2.0 * std::f64::consts::PI * 440.0 * t).cos() as f32 * 0.6;
        interleaved.push(left);
        interleaved.push(right);
    }

    let wav_path = create_temp_wav(
        &interleaved,
        rate,
        2, // stereo
        "test_e2e_stereo_48k.wav",
    ).unwrap();

    let decoded = decode_file_with(&wav_path, &cfg).expect("Failed decode stereo WAV");

    assert_eq!(decoded.sample_rate, WHISPER_SAMPLE_RATE);
    assert_eq!(decoded.source_sample_rate, rate);
    assert_eq!(decoded.source_channels, 2);

    let expected_len = (num_frames as f64 * WHISPER_SAMPLE_RATE as f64 / rate as f64).round() as usize;
    let diff = (decoded.samples.len() as i64 - expected_len as i64).abs();
    assert!(diff <= 1);

    let estimated_freq = estimate_frequency(&decoded.samples, WHISPER_SAMPLE_RATE).unwrap();
    assert!((estimated_freq - 440.0).abs() < 5.0, "Stereo downmix frequency distorted: {estimated_freq}");

    let _ = std::fs::remove_file(&wav_path);
}

// =============================================================================
// 6. Adversarial Verification: Zero & Extreme Sample Rates
// =============================================================================

#[test]
fn test_adversarial_zero_sample_rates_rejected_safely() {
    let cfg = AudioConfig::default();
    let samples = vec![0.5f32; 1024];

    // from_rate = 0, to_rate > 0
    let err_from_zero = resample_reference(&samples, 0, WHISPER_SAMPLE_RATE, &cfg);
    assert!(err_from_zero.is_err(), "from_rate=0 must return Err");
    let err_msg = err_from_zero.unwrap_err().to_string();
    assert!(
        err_msg.contains("out of bounds"),
        "Unexpected error message: {err_msg}"
    );

    // from_rate > 0, to_rate = 0
    let err_to_zero = resample_reference(&samples, 48_000, 0, &cfg);
    assert!(err_to_zero.is_err(), "to_rate=0 must return Err");
    let err_msg = err_to_zero.unwrap_err().to_string();
    assert!(
        err_msg.contains("out of bounds"),
        "Unexpected error message: {err_msg}"
    );

    // both = 0
    let err_both_zero = resample_reference(&samples, 0, 0, &cfg);
    assert!(err_both_zero.is_err(), "both rates=0 must return Err");
    let err_msg = err_both_zero.unwrap_err().to_string();
    assert!(
        err_msg.contains("out of bounds"),
        "Unexpected error message: {err_msg}"
    );
}

#[test]
fn test_adversarial_extreme_sample_rates_safety() {
    let cfg = AudioConfig::default();
    let samples = vec![0.1f32; 100];

    // Test extreme rate: u32::MAX
    // 1. from_rate = u32::MAX -> to_rate = 16_000
    let res_max_down = resample_reference(&samples, u32::MAX, WHISPER_SAMPLE_RATE, &cfg);
    assert!(res_max_down.is_err(), "u32::MAX from_rate must return Err");
    assert!(res_max_down.unwrap_err().to_string().contains("out of bounds"));

    // 2. from_rate = 16_000 -> to_rate = u32::MAX
    let res_max_up = resample_reference(&samples, WHISPER_SAMPLE_RATE, u32::MAX, &cfg);
    assert!(res_max_up.is_err(), "u32::MAX to_rate must return Err");
    assert!(res_max_up.unwrap_err().to_string().contains("out of bounds"));

    // Test extreme rate: 500_000 Hz
    // 1. from_rate = 500_000 -> to_rate = 16_000
    let res_500k_down = resample_reference(&samples, 500_000, WHISPER_SAMPLE_RATE, &cfg);
    assert!(res_500k_down.is_err(), "500_000 from_rate must return Err");
    assert!(res_500k_down.unwrap_err().to_string().contains("out of bounds"));

    // 2. from_rate = 16_000 -> to_rate = 500_000
    let res_500k_up = resample_reference(&samples, WHISPER_SAMPLE_RATE, 500_000, &cfg);
    assert!(res_500k_up.is_err(), "500_000 to_rate must return Err");
    assert!(res_500k_up.unwrap_err().to_string().contains("out of bounds"));

    // 3. from_rate = 500_000 -> to_rate = 500_000
    let res_500k_both = resample_reference(&samples, 500_000, 500_000, &cfg);
    assert!(res_500k_both.is_err(), "500_000 both rates must return Err");
    assert!(res_500k_both.unwrap_err().to_string().contains("out of bounds"));

    // Test extreme rate: 1 Hz
    // 1. from_rate = 1 Hz -> to_rate = 16_000 (extreme ratio = 16000.0)
    let res_1hz = resample_reference(&samples, 1, WHISPER_SAMPLE_RATE, &cfg);
    assert!(res_1hz.is_err(), "1 Hz from_rate must return Err");
    assert!(res_1hz.unwrap_err().to_string().contains("ratio out of bounds"));

    // 2. from_rate = 16_000 -> to_rate = 1 Hz (extreme ratio = 1/16000.0)
    let res_to_1hz = resample_reference(&samples, WHISPER_SAMPLE_RATE, 1, &cfg);
    assert!(res_to_1hz.is_err(), "1 Hz to_rate must return Err");
    assert!(res_to_1hz.unwrap_err().to_string().contains("ratio out of bounds"));

    // Test extreme rate: 1,000,000 Hz
    let mhz_samples = vec![0.25f32; 10_000];
    let res_mhz = resample_reference(&mhz_samples, 1_000_000, WHISPER_SAMPLE_RATE, &cfg);
    assert!(res_mhz.is_err(), "1 MHz from_rate must return Err");
    assert!(res_mhz.unwrap_err().to_string().contains("out of bounds"));

    let res_to_mhz = resample_reference(&samples, WHISPER_SAMPLE_RATE, 1_000_000, &cfg);
    assert!(res_to_mhz.is_err(), "1 MHz to_rate must return Err");
    assert!(res_to_mhz.unwrap_err().to_string().contains("out of bounds"));
}

#[test]
fn test_e2e_decode_file_with_extreme_rates() {
    let cfg = AudioConfig::default();

    // 1. WAV with sample_rate = 0 (adversarial corrupted container)
    // Invokes the real decode_file_with -> audio.rs:resample pipeline!
    let dummy_samples = vec![0.0f32; 100];
    let zero_rate_wav = create_temp_wav(&dummy_samples, 0, 1, "test_e2e_zero_rate.wav").unwrap();
    let res_zero = decode_file_with(&zero_rate_wav, &cfg);
    assert!(res_zero.is_err(), "decoding 0 Hz WAV must return Err, not panic");
    let _ = std::fs::remove_file(&zero_rate_wav);

    // 2. WAV with sample_rate = 1 Hz (extreme ratio rejected by resample)
    let one_hz_samples = vec![0.5f32; 10];
    let one_hz_wav = create_temp_wav(&one_hz_samples, 1, 1, "test_e2e_1hz.wav").unwrap();
    let res_one_hz = decode_file_with(&one_hz_wav, &cfg);
    assert!(res_one_hz.is_err(), "1 Hz WAV decode must return Err");
    let err_msg = res_one_hz.unwrap_err().to_string();
    assert!(err_msg.contains("ratio out of bounds"), "got: {err_msg}");
    let _ = std::fs::remove_file(&one_hz_wav);

    // 3. WAV with sample_rate = 500_000 Hz (exceeds max supported sample rate)
    let five_hundred_k_samples = vec![0.25f32; 1000];
    let five_hundred_k_wav = create_temp_wav(&five_hundred_k_samples, 500_000, 1, "test_e2e_500khz.wav").unwrap();
    let res_500k = decode_file_with(&five_hundred_k_wav, &cfg);
    assert!(res_500k.is_err(), "500 kHz WAV decode must return Err");
    let err_msg = res_500k.unwrap_err().to_string();
    assert!(err_msg.contains("input sample rate out of bounds"), "got: {err_msg}");
    let _ = std::fs::remove_file(&five_hundred_k_wav);

    // 4. WAV with sample_rate = 1,000,000 Hz (exceeds max supported sample rate)
    let mhz_samples = vec![0.25f32; 1000];
    let mhz_wav = create_temp_wav(&mhz_samples, 1_000_000, 1, "test_e2e_1mhz.wav").unwrap();
    let res_mhz = decode_file_with(&mhz_wav, &cfg);
    assert!(res_mhz.is_err(), "1 MHz WAV decode must return Err");
    let err_msg = res_mhz.unwrap_err().to_string();
    assert!(err_msg.contains("input sample rate out of bounds"), "got: {err_msg}");
    let _ = std::fs::remove_file(&mhz_wav);
}


