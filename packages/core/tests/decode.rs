//! The audio path, measured without Whisper in the loop.
//!
//! These tests exist to settle a claim that had lived in a code comment for
//! months: that extracting audio from MP4 "contributes to corrupted
//! transcriptions". That claim was untestable as stated, because it bundled
//! three separable things --- container parsing, AAC decoding, and resampling ---
//! and blamed the file format for whichever one was actually at fault.
//!
//! The fixture set separates them. Each interview supplies three views of the
//! same audio: an original MP4, a re-encoded MP4, and a WAV resampled to 16 kHz
//! mono by an external tool. So original-vs-WAV isolates *our resampler against
//! an external one*, and original-vs-reencode isolates *what the transcode did*.
//! Neither is "MP4 handling" in the abstract, and the distinction decides which
//! knob to reach for.
//!
//! The two interviews are not the same experiment. 108's original is an
//! `mp4v`-era master whose `esds` carries no channel map --- the case that
//! exposed the stereo-read-as-mono bug --- and its re-encode carries an extra
//! AAC generation. 026's files are both H.264 and its "reencoded" file is
//! *larger* than the original, so that pair tests container handling but not
//! generational loss.
//!
//! (108's `truth`/`funky` filenames record the order they were made, not their
//! fidelity. The `funky` one is the original.)
//!
//! Fixtures are gigabytes and live outside the repo, under `AUOHP_FIXTURE_DIR`,
//! `/mnt/s3/fs1/in`, or `$HOME`. Interviews whose media is missing skip with a
//! notice, so `cargo test` stays green on a clean checkout.

use auohp_core::transcription::{decode_file, DecodedAudio};
use std::path::PathBuf;

const TARGET_RATE: u32 = 16_000;

/// Search the fixture roots for `name`.
///
/// Media for different interviews lives in different places (the 108 set beside
/// the home directory, the 026 set on the data volume), so this looks through a
/// list rather than a single directory. `AUOHP_FIXTURE_DIR` is tried first when
/// set, so a caller can always override.
fn fixture(name: &str) -> Option<PathBuf> {
    let mut roots: Vec<PathBuf> = Vec::new();
    if let Ok(d) = std::env::var("AUOHP_FIXTURE_DIR") {
        roots.push(PathBuf::from(d));
    }
    roots.push(PathBuf::from("/mnt/s3/fs1/in"));
    if let Ok(h) = std::env::var("HOME") {
        roots.push(PathBuf::from(h));
    }
    roots.into_iter().map(|r| r.join(name)).find(|p| p.exists())
}

/// One interview's three views of the same audio.
struct Interview {
    id: &'static str,
    original: &'static str,
    reencode: &'static str,
    wav: &'static str,
    /// Expected source rate/channels of the MP4s, before mixdown and resampling.
    source: (u32, usize),
}

/// 108: the original is an `mp4v`-era master whose `esds` carries no channel map
/// -- the case that exposed the stereo-read-as-mono bug. 026: both MP4s are
/// H.264 and the "reencoded" file is *larger* than the original, so that pair is
/// not a generational-loss comparison at all.
const INTERVIEWS: &[Interview] = &[
    Interview { id: "108", original: "108_funky.mp4", reencode: "108_truth.mp4",
                wav: "108_truth.wav", source: (44_100, 2) },
    Interview { id: "026", original: "026_original.mp4", reencode: "026_reencoded.mp4",
                wav: "026_audio.wav", source: (44_100, 2) },
];

// ── Similarity measures ──────────────────────────────────────────────────────

fn rms(x: &[f32]) -> f64 {
    if x.is_empty() {
        return 0.0;
    }
    (x.iter().map(|v| (*v as f64).powi(2)).sum::<f64>() / x.len() as f64).sqrt()
}

/// Normalised cross-correlation of two equal-length slices.
fn ncc(a: &[f32], b: &[f32]) -> f64 {
    let n = a.len().min(b.len());
    if n == 0 {
        return 0.0;
    }
    let (mut num, mut da, mut db) = (0.0f64, 0.0f64, 0.0f64);
    for i in 0..n {
        let (x, y) = (a[i] as f64, b[i] as f64);
        num += x * y;
        da += x * x;
        db += y * y;
    }
    if da == 0.0 || db == 0.0 {
        return 0.0;
    }
    num / (da.sqrt() * db.sqrt())
}

/// Correlation of `a` against `b` shifted by `lag` samples.
fn at_lag(a: &[f32], b: &[f32], lag: isize) -> f64 {
    if lag >= 0 {
        let l = lag as usize;
        if l >= b.len() {
            return f64::NEG_INFINITY;
        }
        ncc(a, &b[l..])
    } else {
        let l = (-lag) as usize;
        if l >= a.len() {
            return f64::NEG_INFINITY;
        }
        ncc(&a[l..], b)
    }
}

/// Best correlation over a lag search, and the lag that achieved it.
fn best_lag(a: &[f32], b: &[f32], max_lag: isize) -> (f64, isize) {
    (-max_lag..=max_lag)
        .map(|lag| (at_lag(a, b, lag), lag))
        .fold((f64::NEG_INFINITY, 0), |best, c| if c.0 > best.0 { c } else { best })
}

const BLOCK: usize = 160; // 10 ms at 16 kHz

/// Per-block RMS --- the signal's amplitude envelope.
///
/// Raw-sample correlation is useless for a coarse search: speech carries energy
/// to 8 kHz, so a misalignment of even one millisecond decorrelates two
/// otherwise-identical streams. The envelope discards phase and keeps only the
/// syllabic shape, which stays correlated across offsets of many samples. Coarse
/// on the envelope, then refine on raw samples, finds a large offset for a tiny
/// fraction of the work a brute-force raw search would need.
fn envelope(x: &[f32]) -> Vec<f32> {
    x.chunks(BLOCK).map(|c| rms(c) as f32).collect()
}

/// Global sample offset between two streams, searched wide.
///
/// Encoders prime their output: AAC typically emits 2112 samples of leading
/// padding at 44.1 kHz, which survives resampling as roughly 766 samples at
/// 16 kHz. That is an expected, constant offset and *not* a defect --- but it is
/// far outside any plausible drift window, so it has to be found and removed
/// before drift can be measured at all.
fn global_offset(a: &[f32], b: &[f32]) -> isize {
    let (ea, eb) = (envelope(a), envelope(b));
    let n = ea.len().min(eb.len());
    // Up to +/- 2 s, expressed in envelope blocks.
    let span = (n / 4).min(200) as isize;
    let (_, coarse) = best_lag(&ea[..n], &eb[..n], span);

    // Refine on raw samples within one block either side of the coarse estimate.
    let centre = coarse * BLOCK as isize;
    let probe = (16_000 * 20).min(a.len()).min(b.len());
    let fine = (-(BLOCK as isize)..=(BLOCK as isize))
        .map(|d| {
            let lag = centre + d;
            (at_lag(&a[..probe], &b[..probe], lag), lag)
        })
        .fold((f64::NEG_INFINITY, centre), |best, c| if c.0 > best.0 { c } else { best });
    fine.1
}

/// Apply a global offset, returning the overlapping portions.
fn shift<'a>(a: &'a [f32], b: &'a [f32], lag: isize) -> (&'a [f32], &'a [f32]) {
    let (a, b) = if lag >= 0 {
        (a, &b[(lag as usize).min(b.len())..])
    } else {
        (&a[((-lag) as usize).min(a.len())..], b)
    };
    let n = a.len().min(b.len());
    (&a[..n], &b[..n])
}

/// Correlation measured independently in successive windows.
///
/// This is the measurement that matters, and the reason a single scalar RMS
/// would have been useless here. If the resampler drops or duplicates samples at
/// chunk boundaries, the two streams do not degrade uniformly --- they slide
/// apart, so early windows correlate well and later ones progressively worse,
/// with a lag that grows. A whole-file average smears exactly that signature
/// into a mediocre number with no shape to it.
/// Correlation and residual lag per window, after the global offset is removed.
///
/// Residual lag is the diagnostic. A constant offset is encoder priming and
/// harmless; a residual that *grows* across the file means samples are being
/// dropped or duplicated as it goes, which is what chunk-boundary mishandling in
/// the resampler would look like. A single whole-file scalar averages that shape
/// away into a mediocre number with nothing to read.
fn windowed(a: &[f32], b: &[f32], windows: usize) -> Vec<(f64, isize)> {
    let n = a.len().min(b.len());
    let w = n / windows;
    (0..windows)
        .map(|i| {
            let (s, e) = (i * w, ((i + 1) * w).min(n));
            best_lag(&a[s..e], &b[s..e], 64)
        })
        .collect()
}

struct Comparison {
    windows: Vec<(f64, isize)>,
    offset: isize,
    mean_ncc: f64,
}

fn report(label: &str, x: &DecodedAudio, y: &DecodedAudio) -> Comparison {
    eprintln!("\n── {label} ─────────────────────────────");
    eprintln!(
        "  lengths {} vs {}  (delta {})",
        x.samples.len(),
        y.samples.len(),
        x.samples.len() as i64 - y.samples.len() as i64
    );
    eprintln!("  rms {:.6} vs {:.6}", rms(&x.samples), rms(&y.samples));

    let offset = global_offset(&x.samples, &y.samples);
    eprintln!(
        "  global offset {offset:+} samples ({:+.1} ms)  <- encoder priming, expected",
        offset as f64 * 1000.0 / TARGET_RATE as f64
    );

    let (xa, ya) = shift(&x.samples, &y.samples, offset);
    let windows = windowed(xa, ya, 8);
    for (i, (c, lag)) in windows.iter().enumerate() {
        eprintln!("  window {i}: ncc {c:+.4}  residual lag {lag:+}");
    }
    let mean_ncc = windows.iter().map(|(c, _)| *c).sum::<f64>() / windows.len() as f64;
    eprintln!("  mean ncc {mean_ncc:.4}");

    Comparison { windows, offset, mean_ncc }
}

// ── Tests ────────────────────────────────────────────────────────────────────

/// Run `f` for every interview whose media is present; skip (with a notice) the
/// ones that are not, so a partial fixture set still exercises what it can.
fn for_each_interview(what: &str, f: impl Fn(&Interview, &DecodedAudio, &DecodedAudio, &DecodedAudio)) {
    let mut ran = 0;
    for iv in INTERVIEWS {
        let (Some(o), Some(r), Some(w)) =
            (fixture(iv.original), fixture(iv.reencode), fixture(iv.wav))
        else {
            eprintln!("SKIP {}: media for interview {} not found", what, iv.id);
            continue;
        };
        eprintln!("\n######## interview {} :: {what}", iv.id);
        let (od, rd, wd) = (
            decode_file(&o).expect("decode original"),
            decode_file(&r).expect("decode re-encode"),
            decode_file(&w).expect("decode wav"),
        );
        f(iv, &od, &rd, &wd);
        ran += 1;
    }
    if ran == 0 {
        eprintln!("SKIP: no interview media found (set AUOHP_FIXTURE_DIR)");
    }
}

/// Both MP4s must expose their declared source format before the transforms and
/// 16 kHz mono after; the WAV must report that it bypassed both.
#[test]
fn decode_reports_expected_stream_params() {
    for_each_interview("stream params", |iv, o, r, w| {
        for (label, d) in [("original", o), ("re-encode", r)] {
            assert_eq!(d.sample_rate, TARGET_RATE, "{label}");
            assert_eq!((d.source_sample_rate, d.source_channels), iv.source, "{label}");
        }
        assert_eq!(w.sample_rate, TARGET_RATE);
        assert_eq!(
            (w.source_sample_rate, w.source_channels), (TARGET_RATE, 1),
            "the WAV must bypass mix_to_mono and resample -- that is its whole job as a control"
        );
    });
}

/// Guards the track-selection defect: these containers lead with a video track,
/// so "first track with a non-null codec" picked the audio by luck. It also
/// guards the channel-map defect -- 108's master reports no channel count, and
/// reading that as mono fed interleaved stereo to the resampler.
#[test]
fn containers_select_the_audio_track() {
    for_each_interview("track selection", |iv, o, _r, _w| {
        assert!(
            o.samples.len() > TARGET_RATE as usize * 60,
            "interview {}: decoded only {} samples -- what picking the wrong track looks like",
            iv.id, o.samples.len()
        );
        assert!(rms(&o.samples) > 1e-4, "interview {}: decoded audio is silent", iv.id);
    });
}

/// The core measurement: same source audio, two resamplers.
///
/// A failure here does not indict "MP4 handling" -- both paths decode the same
/// AAC. It indicts our `rubato` configuration against the external tool's.
#[test]
fn mp4_matches_externally_resampled_wav() {
    for_each_interview("resampler vs reference", |iv, o, _r, w| {
        let c = report("original MP4 vs externally resampled WAV", o, w);

        let delta = (o.samples.len() as i64 - w.samples.len() as i64).abs();
        assert!(
            delta < TARGET_RATE as i64,
            "interview {}: duration mismatch of {} samples ({:.2}s) -- the resampler is \
             losing or inventing audio, not merely colouring it",
            iv.id, delta, delta as f64 / TARGET_RATE as f64
        );
        assert!(
            c.mean_ncc > 0.5,
            "interview {}: mean correlation {:.4} after removing a {:+}-sample global offset; \
             the two decodes are not the same audio",
            iv.id, c.mean_ncc, c.offset
        );

        // Residual drift, not absolute offset, is the resampler diagnostic.
        let lags: Vec<isize> = c.windows.iter().map(|(_, l)| *l).collect();
        let spread = lags.iter().max().unwrap() - lags.iter().min().unwrap();
        assert!(
            spread <= 8,
            "interview {}: alignment drifts across the file (residual lags {lags:?}) -- \
             symptomatic of samples dropped or duplicated at resample chunk boundaries",
            iv.id
        );
    });
}

/// Compares the two MP4s against the WAV reference.
///
/// For 108 this measures generational loss (the re-encode carries an extra AAC
/// generation). For 026 the "reencoded" file is larger than the original, so it
/// is not a lossy transcode and no ordering is predicted. Either way the numbers
/// are reported rather than asserted -- which way they fall is the experiment's
/// outcome, and asserting it would be asserting the hypothesis.
#[test]
fn reencode_and_original_are_compared_against_the_reference() {
    for_each_interview("generational loss", |iv, o, r, w| {
        let mo = report("original vs WAV", o, w).mean_ncc;
        let mr = report("re-encode vs WAV", r, w).mean_ncc;
        eprintln!(
            "\n  interview {}: mean ncc  original {mo:.4}   re-encode {mr:.4}   delta {:+.4}",
            iv.id, mo - mr
        );
        assert!(mo > 0.5 && mr > 0.5, "interview {}: an MP4 does not match the WAV", iv.id);
        if mr > mo {
            eprintln!("  NOTE: the re-encode matches the WAV *better* than the original does.");
        }
    });
}

/// Resolves each WAV's provenance, which decides how much weight it carries as a
/// control: taken from the original it is the cleanest reference available;
/// taken from the re-encode it inherits that file's extra generation.
#[test]
fn wav_provenance_is_identifiable() {
    for_each_interview("wav provenance", |iv, o, r, w| {
        let score = |x: &DecodedAudio| {
            let off = global_offset(&w.samples, &x.samples);
            let (wa, xa) = shift(&w.samples, &x.samples, off);
            let n = (16_000 * 300).min(wa.len());
            (ncc(&wa[..n], &xa[..n]), off)
        };
        let (to_orig, off_o) = score(o);
        let (to_re, off_r) = score(r);
        eprintln!("\n── interview {} WAV provenance ──", iv.id);
        eprintln!("  ncc to original  {to_orig:.5}  (offset {off_o:+})");
        eprintln!("  ncc to re-encode {to_re:.5}  (offset {off_r:+})");
        eprintln!("  => extracted from the {}",
            if to_orig >= to_re { "ORIGINAL" } else { "RE-ENCODE" });
    });
}
