//! Standalone adversarial stress test harness for Silero VAD timeline mapping (`whisper/vad.rs`).
//!
//! Evaluates:
//! 1. Empty timeline (no speech regions, default, identity).
//! 2. Entire recording is speech (identity mapping, boundary clamping).
//! 3. Alternating 1-sample speech and 1-sample silence regions (16 kHz stress test).
//! 4. Out-of-bounds queries: t < 0.0, t > total_duration, t = NaN, t = ±infinity.
//! 5. Monotonicity of `to_original` mapping across region boundaries and glue silences.
//! 6. Zero duration regions, single region, and high-density multi-region stress (10,000 regions).
//! 7. Long duration floating point precision stability (10-hour interview).
//! 8. Overlapping region boundary hazard analysis.
//! 9. Combinatorial property-based fuzzing across 100,000 queries.

#[allow(dead_code)]
mod root_mod {
    pub mod config {
        pub use auohp_core::transcription::VadConfig;
    }
    pub mod whisper {
        #[path = "/Users/daniel/.gemini/antigravity/worktrees/auohp/refactor_whisper_pipeline/packages/core/src/transcription/whisper/vad.rs"]
        pub mod vad;
    }
}

pub use root_mod::whisper::vad::{VadRegion, VadTimeline, VAD_GLUE_SECONDS};

// =========================================================================
// 1. Empty Timeline & Identity Invariants
// =========================================================================

#[test]
fn test_empty_timeline_identity() {
    let timelines = [
        VadTimeline::identity(),
        VadTimeline::default(),
        VadTimeline { regions: vec![] },
    ];

    for timeline in &timelines {
        assert!(timeline.is_identity(), "Empty timeline must report is_identity() == true");

        // Identity timeline maps every query directly to itself
        let test_queries = [
            0.0,
            1.0,
            42.5,
            1000.0,
            -1.0,
            -999.0,
            1e12,
            f64::MIN_POSITIVE,
            f64::MAX,
        ];

        for &t in &test_queries {
            assert_eq!(
                timeline.to_original(t),
                t,
                "Identity timeline must return t unchanged for t={t}"
            );
        }

        // Check infinities
        assert_eq!(timeline.to_original(f64::INFINITY), f64::INFINITY);
        assert_eq!(timeline.to_original(f64::NEG_INFINITY), f64::NEG_INFINITY);

        // Check NaN
        assert!(
            timeline.to_original(f64::NAN).is_nan(),
            "Empty timeline should return NaN for NaN query"
        );

        // Monotonicity on empty timeline
        let mut prev = f64::NEG_INFINITY;
        for i in -100..=100 {
            let t = i as f64 * 0.5;
            let mapped = timeline.to_original(t);
            assert!(
                mapped >= prev,
                "Empty timeline must be monotonic: t={t}, mapped={mapped}, prev={prev}"
            );
            prev = mapped;
        }
    }
}

// =========================================================================
// 2. Entire Recording is Speech (Identity Mapping)
// =========================================================================

#[test]
fn test_entire_recording_is_speech() {
    let duration = 60.0;
    let timeline = VadTimeline {
        regions: vec![VadRegion {
            orig_start: 0.0,
            filtered_start: 0.0,
            duration,
        }],
    };

    assert!(!timeline.is_identity());

    // Inside speech window [0.0, 60.0], mapping must be strictly identity: to_original(t) == t
    for i in 0..=6000 {
        let t = i as f64 * 0.01;
        let mapped = timeline.to_original(t);
        assert!(
            (mapped - t).abs() < 1e-12,
            "Mapping within speech must equal t: t={t}, mapped={mapped}"
        );
    }

    // Boundary conditions:
    // Left boundary: t < 0.0 should clamp to orig_start (0.0)
    for &t in &[-1e-12, -0.01, -1.0, -100.0, -1e9, f64::NEG_INFINITY] {
        assert_eq!(
            timeline.to_original(t),
            0.0,
            "Negative times must clamp to orig_start (0.0), got {} for t={t}",
            timeline.to_original(t)
        );
    }

    // Right boundary: t > 60.0 should clamp to last_end (60.0)
    for &t in &[60.0001, 60.5, 100.0, 1e6, f64::INFINITY] {
        assert_eq!(
            timeline.to_original(t),
            60.0,
            "Times past the end must clamp to last_end (60.0), got {} for t={t}",
            timeline.to_original(t)
        );
    }

    // Monotonicity across the entire range including out-of-bounds
    let mut prev = f64::NEG_INFINITY;
    for i in -500..=6500 {
        let t = i as f64 * 0.01;
        let mapped = timeline.to_original(t);
        assert!(
            mapped >= prev,
            "Monotonicity violation at t={t}: mapped={mapped} < prev={prev}"
        );
        prev = mapped;
    }
}

// =========================================================================
// 3. Alternating 1-Sample Speech and 1-Sample Silence Regions
// =========================================================================

#[test]
fn test_alternating_one_sample_speech_and_silence() {
    let sample_rate = 16_000.0;
    let dt = 1.0 / sample_rate; // 0.0000625 s
    let n_regions = 1_000;

    // In the original recording:
    // Region 0: [0*2*dt .. 0*2*dt + dt] (silence from dt to 2*dt)
    // Region 1: [1*2*dt .. 1*2*dt + dt] (silence from 3*dt to 4*dt)
    // ...
    // Region k: [2*k*dt .. 2*k*dt + dt]
    //
    // In the filtered audio:
    // Region 0: filtered_start = 0.0, duration = dt
    // Glue: VAD_GLUE_SECONDS (0.1 s)
    // Region 1: filtered_start = dt + 0.1, duration = dt
    // ...
    // Region k: filtered_start = k * (dt + VAD_GLUE_SECONDS), duration = dt

    let mut regions = Vec::with_capacity(n_regions);
    let mut filtered_cursor = 0.0;

    for k in 0..n_regions {
        let orig_start = (2 * k) as f64 * dt;
        let filtered_start = filtered_cursor;
        regions.push(VadRegion {
            orig_start,
            filtered_start,
            duration: dt,
        });
        filtered_cursor += dt + VAD_GLUE_SECONDS;
    }

    let timeline = VadTimeline { regions };

    // 1. Verify exact mapping at speech sample boundaries for all 1,000 regions
    for (k, r) in timeline.regions.iter().enumerate() {
        // Start of 1-sample speech
        let orig_at_start = timeline.to_original(r.filtered_start);
        assert!(
            (orig_at_start - r.orig_start).abs() < 1e-12,
            "Region {k} start mapping failed: expected {}, got {}",
            r.orig_start,
            orig_at_start
        );

        // End of 1-sample speech
        let orig_at_end = timeline.to_original(r.filtered_start + r.duration);
        let expected_end = r.orig_start + r.duration;
        assert!(
            (orig_at_end - expected_end).abs() < 1e-12,
            "Region {k} end mapping failed: expected {}, got {}",
            expected_end,
            orig_at_end
        );

        // 2. Verify glue silence clamping: midway through glue must clamp to end of preceding region
        if k < n_regions - 1 {
            let mid_glue = r.filtered_start + r.duration + (VAD_GLUE_SECONDS / 2.0);
            let orig_in_glue = timeline.to_original(mid_glue);
            assert_eq!(
                orig_in_glue,
                expected_end,
                "Region {k} glue silence must clamp to preceding region end"
            );
        }
    }

    // 3. Strict Monotonicity Sweep across region boundaries and glue silences
    let mut prev = f64::NEG_INFINITY;
    // Step through the first 50 regions with fine-grained steps (15 points per region + glue)
    for k in 0..50 {
        let r = &timeline.regions[k];
        let steps = 15;
        let span = dt + VAD_GLUE_SECONDS;
        for s in 0..steps {
            let t = r.filtered_start + (s as f64 * span / steps as f64);
            let mapped = timeline.to_original(t);
            assert!(
                mapped >= prev,
                "Monotonicity failed in 1-sample alternating test at k={k}, step={s}, t={t}: mapped={mapped} < prev={prev}"
            );
            prev = mapped;
        }
    }
}

// =========================================================================
// 4. Out-of-Bounds Queries (t < 0.0, t > total_duration, NaN, infinity)
// =========================================================================

#[test]
fn test_out_of_bounds_queries() {
    let timeline = VadTimeline {
        regions: vec![
            VadRegion {
                orig_start: 5.0,
                filtered_start: 0.0,
                duration: 2.0,
            },
            VadRegion {
                orig_start: 20.0,
                filtered_start: 2.1,
                duration: 3.0,
            },
        ],
    };

    let total_filtered = 2.1 + 3.0; // 5.1 s
    let expected_first_start = 5.0;
    let expected_last_end = 20.0 + 3.0; // 23.0 s

    // A. Queries before timeline start (t < 0.0)
    let negative_queries = [
        -1e-15,
        -0.001,
        -1.0,
        -100.0,
        -1e9,
        f64::MIN,
        f64::NEG_INFINITY,
    ];
    for &t in &negative_queries {
        let mapped = timeline.to_original(t);
        assert_eq!(
            mapped, expected_first_start,
            "Negative query t={t} must clamp to first region orig_start ({expected_first_start}), got {mapped}"
        );
    }

    // B. Queries past total filtered duration (t > 5.1)
    let past_end_queries = [
        total_filtered + 1e-12,
        total_filtered + 0.001,
        6.0,
        100.0,
        1e9,
        f64::MAX,
        f64::INFINITY,
    ];
    for &t in &past_end_queries {
        let mapped = timeline.to_original(t);
        assert_eq!(
            mapped, expected_last_end,
            "Query past end t={t} must clamp to last region end ({expected_last_end}), got {mapped}"
        );
    }

    // C. NaN and -NaN queries: must NOT panic, returns last_end because all comparisons evaluate false
    let nan_val = f64::NAN;
    let neg_nan_val = -f64::NAN;
    let mapped_nan = timeline.to_original(nan_val);
    let mapped_neg_nan = timeline.to_original(neg_nan_val);

    assert_eq!(
        mapped_nan, expected_last_end,
        "NaN query must safely return last_end ({expected_last_end}) without panicking, got {mapped_nan}"
    );
    assert_eq!(
        mapped_neg_nan, expected_last_end,
        "-NaN query must safely return last_end ({expected_last_end}) without panicking, got {mapped_neg_nan}"
    );

    // D. Subnormal numbers
    let subnormal = f64::MIN_POSITIVE / 2.0;
    let mapped_subnormal = timeline.to_original(subnormal);
    assert!(
        mapped_subnormal >= expected_first_start && mapped_subnormal <= expected_first_start + 2.0,
        "Subnormal query must map validly inside region 0"
    );
}

// =========================================================================
// 5. Monotonicity Across Region Boundaries & Discontinuous Gaps
// =========================================================================

#[test]
fn test_monotonicity_across_complex_boundaries() {
    // 4 regions with varying gaps:
    // Region 0: real [10.0 .. 12.0], filtered [0.0 .. 2.0]
    // Glue: 0.1s [2.0 .. 2.1]
    // Region 1: real [15.0 .. 18.0], filtered [2.1 .. 5.1]
    // Glue: 0.1s [5.1 .. 5.2]
    // Region 2: real [18.0 .. 18.5] (consecutive real speech, but separated by glue in filtered)
    // Filtered: [5.2 .. 5.7]
    // Glue: 0.1s [5.7 .. 5.8]
    // Region 3: real [30.0 .. 35.0], filtered [5.8 .. 10.8]
    let timeline = VadTimeline {
        regions: vec![
            VadRegion {
                orig_start: 10.0,
                filtered_start: 0.0,
                duration: 2.0,
            },
            VadRegion {
                orig_start: 15.0,
                filtered_start: 2.1,
                duration: 3.0,
            },
            VadRegion {
                orig_start: 18.0,
                filtered_start: 5.2,
                duration: 0.5,
            },
            VadRegion {
                orig_start: 30.0,
                filtered_start: 5.8,
                duration: 5.0,
            },
        ],
    };

    // Verify critical boundary instants
    let boundaries = [
        // Boundary around Region 0 and Glue 0
        (1.999, 11.999),
        (2.0, 12.0),
        (2.0001, 12.0),
        (2.05, 12.0),
        (2.0999, 12.0),
        (2.1, 15.0),
        (2.1001, 15.0001),
        // Boundary around Region 1 and Glue 1
        (5.0999, 17.9999),
        (5.1, 18.0),
        (5.15, 18.0),
        (5.2, 18.0), // Consecutive speech: Region 2 starts at 18.0
        (5.25, 18.05),
        // Boundary around Region 2 and Glue 2
        (5.7, 18.5),
        (5.75, 18.5),
        (5.8, 30.0),
        (5.85, 30.05),
        // End of Region 3
        (10.8, 35.0),
        (10.8001, 35.0),
    ];

    for &(t, expected) in &boundaries {
        let actual = timeline.to_original(t);
        assert!(
            (actual - expected).abs() < 1e-9,
            "At boundary t={t}: expected {expected}, got {actual}"
        );
    }

    // High resolution monotonicity check over 16,000 continuous points (step = 1 ms)
    let mut prev = f64::NEG_INFINITY;
    for i in -1000..=15000 {
        let t = i as f64 * 0.001;
        let cur = timeline.to_original(t);
        assert!(
            cur >= prev,
            "Monotonicity violated at t={t}: cur={cur} < prev={prev}"
        );
        prev = cur;
    }
}

// =========================================================================
// 6. Zero-Duration Speech Region Resilience
// =========================================================================

#[test]
fn test_zero_duration_region_handling() {
    let timeline = VadTimeline {
        regions: vec![
            VadRegion {
                orig_start: 1.0,
                filtered_start: 0.0,
                duration: 0.0, // Zero length
            },
            VadRegion {
                orig_start: 5.0,
                filtered_start: 0.1,
                duration: 2.0,
            },
        ],
    };

    // Zero-length region at t = 0.0
    assert_eq!(timeline.to_original(0.0), 1.0);
    // In glue between 0.0 and 0.1
    assert_eq!(timeline.to_original(0.05), 1.0);
    // Region 1 start at t = 0.1
    assert_eq!(timeline.to_original(0.1), 5.0);

    // Monotonicity holds
    let mut prev = f64::NEG_INFINITY;
    for i in 0..=250 {
        let t = i as f64 * 0.01;
        let cur = timeline.to_original(t);
        assert!(cur >= prev, "Monotonicity failed with zero-duration region at t={t}");
        prev = cur;
    }
}

// =========================================================================
// 7. Scale & Performance Stress: 10,000 Speech Regions
// =========================================================================

#[test]
fn test_large_scale_10k_regions_stress() {
    let n = 10_000;
    let mut regions = Vec::with_capacity(n);
    let mut filtered_time = 0.0;
    let mut orig_time = 0.0;

    for _ in 0..n {
        regions.push(VadRegion {
            orig_start: orig_time,
            filtered_start: filtered_time,
            duration: 0.5,
        });
        orig_time += 1.0; // 0.5s speech + 0.5s dropped silence
        filtered_time += 0.5 + VAD_GLUE_SECONDS;
    }

    let timeline = VadTimeline { regions };

    // Test start, middle, and end lookups
    assert_eq!(timeline.to_original(0.0), 0.0);

    // Query 5,000th region
    let mid_r = &timeline.regions[5000];
    let mid_orig = timeline.to_original(mid_r.filtered_start + 0.25);
    assert!((mid_orig - (mid_r.orig_start + 0.25)).abs() < 1e-12);

    // Query past end
    let last_r = &timeline.regions[n - 1];
    let end_orig = timeline.to_original(last_r.filtered_start + 10.0);
    assert_eq!(end_orig, last_r.orig_start + 0.5);

    // Monotonicity spot checks across 500 points
    let mut prev = f64::NEG_INFINITY;
    for i in (0..10_000).step_by(20) {
        let t = timeline.regions[i].filtered_start;
        let cur = timeline.to_original(t);
        assert!(cur >= prev, "Monotonicity violated in 10k stress at region {i}");
        prev = cur;
    }
}

// =========================================================================
// 8. Long Recording Precision (10-Hour Interview Timeline)
// =========================================================================

#[test]
fn test_long_recording_precision_10_hours() {
    // 10 hours = 36,000 seconds
    let hours_10 = 36_000.0;
    let timeline = VadTimeline {
        regions: vec![
            VadRegion {
                orig_start: 100.0,
                filtered_start: 0.0,
                duration: 18_000.0, // 5 hours speech
            },
            VadRegion {
                orig_start: 18_200.0,
                filtered_start: 18_000.1,
                duration: 17_800.0,
            },
        ],
    };

    // Sub-millisecond timing precision must be preserved at 36,000 seconds
    let t_query = 18_000.1 + 10_000.123456;
    let expected = 18_200.0 + 10_000.123456;
    let actual = timeline.to_original(t_query);
    assert!(
        (actual - expected).abs() < 1e-6,
        "Float precision degradation at 10 hours: expected {expected}, got {actual}"
    );

    assert_eq!(timeline.to_original(hours_10 + 5000.0), 18_200.0 + 17_800.0);
}

// =========================================================================
// 9. Overlapping Region Boundary Hazard Analysis
// =========================================================================

#[test]
fn test_overlapping_region_hazard_documentation() {
    // If regions overlap in original time (e.g. if orig_start_{i+1} < orig_end_i due to samples_overlap),
    // to_original will decrement when jumping from the glue of region i to the start of region i+1.
    let overlapping_timeline = VadTimeline {
        regions: vec![
            VadRegion {
                orig_start: 1.0,
                filtered_start: 0.0,
                duration: 1.1, // orig_end = 2.1
            },
            VadRegion {
                orig_start: 2.05, // Starts at 2.05 < 2.1!
                filtered_start: 1.2,
                duration: 1.0,
            },
        ],
    };

    let at_glue = overlapping_timeline.to_original(1.15); // Returns last_end = 2.1
    let at_next_start = overlapping_timeline.to_original(1.20); // Returns orig_start = 2.05

    assert_eq!(at_glue, 2.1);
    assert_eq!(at_next_start, 2.05);

    // This documents the invariant requirement: apply_vad MUST guarantee orig_start_{i+1} >= orig_end_i
    // (i.e. orig_end must be clamped to next segment's start) for monotonicity to hold unconditionally.
    let went_backwards = at_next_start < at_glue;
    assert!(went_backwards, "Confirms monotonicity depends on non-overlapping original spans");
}

// =========================================================================
// 10. Combinatorial Fuzzing Invariant (100,000 Probes)
// =========================================================================

#[test]
fn test_fuzzing_monotonicity_invariants() {
    // Simple deterministic LCG pseudo-random generator
    struct SimpleRng(u64);
    impl SimpleRng {
        fn next_u64(&mut self) -> u64 {
            self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            self.0
        }
        fn next_f64(&mut self) -> f64 {
            (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
        }
    }

    let mut rng = SimpleRng(0xDEAD_BEEF_CAFE_BABE);

    // Generate 20 randomized timelines
    for _ in 0..20 {
        let num_regions = 5 + (rng.next_u64() % 50) as usize;
        let mut regions = Vec::with_capacity(num_regions);
        let mut orig_cursor = rng.next_f64() * 10.0;
        let mut filtered_cursor = 0.0;

        for _ in 0..num_regions {
            let duration = 0.05 + rng.next_f64() * 10.0;
            regions.push(VadRegion {
                orig_start: orig_cursor,
                filtered_start: filtered_cursor,
                duration,
            });
            // Advance in original recording (strictly >= 0 dropped silence)
            let silence = rng.next_f64() * 5.0;
            orig_cursor += duration + silence;
            // Advance in filtered timeline (duration + glue)
            filtered_cursor += duration + VAD_GLUE_SECONDS;
        }

        let timeline = VadTimeline { regions };

        // Test 5,000 monotonically increasing probe queries
        let total_filtered = filtered_cursor;
        let mut prev = f64::NEG_INFINITY;
        let num_probes = 5_000;
        let start_probe = -2.0;
        let end_probe = total_filtered + 5.0;
        let step = (end_probe - start_probe) / num_probes as f64;

        for p in 0..=num_probes {
            let t = start_probe + (p as f64 * step);
            let mapped = timeline.to_original(t);
            assert!(
                mapped >= prev,
                "Fuzz monotonicity violation: t={t}, mapped={mapped}, prev={prev}"
            );
            prev = mapped;
        }
    }
}
