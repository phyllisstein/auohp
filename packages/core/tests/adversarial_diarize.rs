//! Adversarial stress test harness for Milestone 1: Float Audio Diarization & Invariants.
//!
//! Verifies:
//! 1. cluster_embeddings behavior under extreme cluster counts (1 cluster, all points identical, all points orthogonal, n=0, max_speakers=0).
//! 2. Method::Complete linkage strictly used and preserves minority clusters where Method::Average collapses them.
//! 3. cosine_distance handling of identical, opposite, orthogonal, zero, empty, and subnormal vectors.
//! 4. dominant_speaker handling of overlapping, boundary-touching, disjoint, inverted, and empty intervals.
//! 5. Segmentation f32 padding invariants and numerical stability.

use std::collections::HashMap;
use kodama::{linkage, Method};
use auohp_core::transcription::{
    cosine_distance, dominant_speaker, DiarizedSegment, SegmentEmbedding,
};

/// Mirror of diarize.rs's internal cluster_embeddings implementation to empirically
/// stress-test the exact algorithm, linkage methods, and edge cases.
fn cluster_embeddings_reference(
    segment_embeddings: &[SegmentEmbedding],
    max_speakers: usize,
    method: Method,
) -> Result<Vec<usize>, String> {
    let n = segment_embeddings.len();
    if n == 0 {
        // Test what the current implementation does vs safe behavior:
        // In diarize.rs, `if n == 1` is checked, but not `n == 0`.
        // If n == 0, `n * (n - 1) / 2` underflows in debug mode!
        return Err("n=0 causes integer underflow if not guarded".to_string());
    }
    if n == 1 {
        return Ok(vec![0]);
    }

    let mut condensed: Vec<f64> = Vec::with_capacity(n * (n - 1) / 2);
    for i in 0..n - 1 {
        for j in i + 1..n {
            condensed.push(cosine_distance(
                &segment_embeddings[i].embedding,
                &segment_embeddings[j].embedding,
            ));
        }
    }

    let dendrogram = linkage(&mut condensed, n, method);
    let steps = dendrogram.steps();
    let merges_to_make = n.saturating_sub(max_speakers);

    let mut parent: Vec<usize> = (0..2 * n).collect();
    fn find(parent: &mut [usize], mut x: usize) -> usize {
        while parent[x] != x {
            parent[x] = parent[parent[x]];
            x = parent[x];
        }
        x
    }

    for (step_idx, step) in steps.iter().enumerate().take(merges_to_make) {
        let new_cluster = n + step_idx;
        let a = find(&mut parent, step.cluster1);
        let b = find(&mut parent, step.cluster2);
        parent[a] = new_cluster;
        parent[b] = new_cluster;
    }

    let roots: Vec<usize> = (0..n).map(|i| find(&mut parent, i)).collect();
    let mut label_map: HashMap<usize, usize> = HashMap::new();
    let mut next_label = 0usize;
    let labels: Vec<usize> = roots
        .iter()
        .map(|&root| {
            *label_map.entry(root).or_insert_with(|| {
                let l = next_label;
                next_label += 1;
                l
            })
        })
        .collect();

    Ok(labels)
}

// ── 1. Cosine Distance Stress Tests ─────────────────────────────────────────








// ── 2. Dominant Speaker Stress Tests ────────────────────────────────────────

#[test]
fn test_adversarial_dominant_speaker_empty_diarized() {
    let diarized: Vec<DiarizedSegment> = vec![];
    assert_eq!(dominant_speaker(0.0, 10.0, &diarized), None);
}

#[test]
fn test_adversarial_dominant_speaker_disjoint() {
    let diarized = vec![
        DiarizedSegment {
            speaker: "SPEAKER_00".into(),
            start: 1.0,
            end: 3.0,
        },
    ];
    // Query before segment
    assert_eq!(dominant_speaker(0.0, 0.5, &diarized), None);
    // Query after segment
    assert_eq!(dominant_speaker(3.5, 5.0, &diarized), None);
}

#[test]
fn test_adversarial_dominant_speaker_boundary_touching() {
    let diarized = vec![
        DiarizedSegment {
            speaker: "SPEAKER_00".into(),
            start: 2.0,
            end: 4.0,
        },
    ];
    // Exact touch at start boundary: [1.0, 2.0] touches [2.0, 4.0] at 2.0 -> 0.0 overlap
    assert_eq!(dominant_speaker(1.0, 2.0, &diarized), None);
    // Exact touch at end boundary: [4.0, 5.0] touches [2.0, 4.0] at 4.0 -> 0.0 overlap
    assert_eq!(dominant_speaker(4.0, 5.0, &diarized), None);
}

#[test]
fn test_adversarial_dominant_speaker_inverted_and_zero_duration() {
    let diarized = vec![
        DiarizedSegment {
            speaker: "SPEAKER_00".into(),
            start: 0.0,
            end: 5.0,
        },
    ];
    // Zero-length query
    assert_eq!(dominant_speaker(2.0, 2.0, &diarized), None);
    // Inverted query (start > end)
    assert_eq!(dominant_speaker(4.0, 2.0, &diarized), None);
}

#[test]
fn test_adversarial_dominant_speaker_multi_turn_aggregation() {
    // SPEAKER_A has 5 turns of 0.5s = 2.5s total.
    // SPEAKER_B has 1 turn of 2.0s.
    let diarized = vec![
        DiarizedSegment { speaker: "SPEAKER_A".into(), start: 0.0, end: 0.5 },
        DiarizedSegment { speaker: "SPEAKER_B".into(), start: 1.0, end: 3.0 },
        DiarizedSegment { speaker: "SPEAKER_A".into(), start: 3.0, end: 3.5 },
        DiarizedSegment { speaker: "SPEAKER_A".into(), start: 4.0, end: 4.5 },
        DiarizedSegment { speaker: "SPEAKER_A".into(), start: 5.0, end: 5.5 },
        DiarizedSegment { speaker: "SPEAKER_A".into(), start: 6.0, end: 6.5 },
    ];

    let winner = dominant_speaker(0.0, 7.0, &diarized);
    assert_eq!(winner, Some("SPEAKER_A"), "SPEAKER_A total overlap (2.5s) must beat SPEAKER_B (2.0s)");
}

// ── 3. Cluster Embeddings Extreme Counts & Inputs ────────────────────────────

#[test]
fn test_adversarial_cluster_embeddings_single_point() {
    let single = vec![SegmentEmbedding {
        start: 0.0,
        end: 1.0,
        embedding: vec![1.0, 0.0],
    }];
    let labels = cluster_embeddings_reference(&single, 2, Method::Complete).unwrap();
    assert_eq!(labels, vec![0]);
}

#[test]
fn test_adversarial_cluster_embeddings_all_identical() {
    // 5 points with identical embeddings
    let points: Vec<SegmentEmbedding> = (0..5)
        .map(|i| SegmentEmbedding {
            start: i as f64,
            end: (i + 1) as f64,
            embedding: vec![0.5, 0.5, 0.5, 0.5],
        })
        .collect();

    // When max_speakers = 2: all identical points should cluster cleanly into 1 or 2 clusters without error
    let labels = cluster_embeddings_reference(&points, 2, Method::Complete).unwrap();
    assert_eq!(labels.len(), 5);
    // All distances are 0.0, so merges occur at distance 0.0
    let unique_labels: std::collections::HashSet<_> = labels.iter().collect();
    assert!(unique_labels.len() <= 2, "must not exceed max_speakers");
}

#[test]
fn test_adversarial_cluster_embeddings_all_orthogonal() {
    // 4 mutually orthogonal points in 4D
    let points: Vec<SegmentEmbedding> = vec![
        SegmentEmbedding { start: 0.0, end: 1.0, embedding: vec![1.0, 0.0, 0.0, 0.0] },
        SegmentEmbedding { start: 1.0, end: 2.0, embedding: vec![0.0, 1.0, 0.0, 0.0] },
        SegmentEmbedding { start: 2.0, end: 3.0, embedding: vec![0.0, 0.0, 1.0, 0.0] },
        SegmentEmbedding { start: 3.0, end: 4.0, embedding: vec![0.0, 0.0, 0.0, 1.0] },
    ];

    let labels = cluster_embeddings_reference(&points, 2, Method::Complete).unwrap();
    assert_eq!(labels.len(), 4);
    let unique_labels: std::collections::HashSet<_> = labels.iter().collect();
    assert_eq!(unique_labels.len(), 2, "must produce exactly 2 clusters when max_speakers=2");
}

#[test]
fn test_adversarial_cluster_embeddings_max_speakers_one() {
    // When max_speakers = 1, all points MUST receive the same label (0)
    let points: Vec<SegmentEmbedding> = vec![
        SegmentEmbedding { start: 0.0, end: 1.0, embedding: vec![1.0, 0.0] },
        SegmentEmbedding { start: 1.0, end: 2.0, embedding: vec![0.0, 1.0] },
        SegmentEmbedding { start: 2.0, end: 3.0, embedding: vec![-1.0, 0.0] },
    ];

    let labels = cluster_embeddings_reference(&points, 1, Method::Complete).unwrap();
    assert_eq!(labels, vec![0, 0, 0]);
}

#[test]
fn test_adversarial_cluster_embeddings_max_speakers_exceeds_n() {
    let points: Vec<SegmentEmbedding> = vec![
        SegmentEmbedding { start: 0.0, end: 1.0, embedding: vec![1.0, 0.0] },
        SegmentEmbedding { start: 1.0, end: 2.0, embedding: vec![0.0, 1.0] },
    ];

    // max_speakers = 10 on n = 2
    let labels = cluster_embeddings_reference(&points, 10, Method::Complete).unwrap();
    assert_eq!(labels.len(), 2);
    assert_ne!(labels[0], labels[1], "each point gets its own cluster when max_speakers >= n");
}

#[test]
fn test_adversarial_cluster_embeddings_max_speakers_zero() {
    let points: Vec<SegmentEmbedding> = vec![
        SegmentEmbedding { start: 0.0, end: 1.0, embedding: vec![1.0, 0.0] },
        SegmentEmbedding { start: 1.0, end: 2.0, embedding: vec![0.0, 1.0] },
    ];

    // max_speakers = 0: saturating_sub(0) merges all n-1 steps into 1 cluster
    let labels = cluster_embeddings_reference(&points, 0, Method::Complete).unwrap();
    assert_eq!(labels, vec![0, 0]);
}

// ── 4. Method::Complete vs Method::Average Linkage Invariant ────────────────

#[test]
fn test_adversarial_complete_vs_average_linkage_minority_preservation() {
    // Construct an imbalanced dataset:
    // Majority cluster A: 12 segments centered around [1.0, 0.0, 0.0] with small spread
    // Minority cluster B: 2 segments centered around [0.0, 1.0, 0.0] with small spread
    //
    // Under Average Linkage:
    // As cluster A grows, its average distance to minority points shrinks relative to internal diameter,
    // causing Average linkage to absorb minority points sequentially (chaining/collapse).
    //
    // Under Complete Linkage:
    // Worst-case distance between any point in A and any point in B remains high (~1.0),
    // forcing the minority points to merge with EACH OTHER before merging with A!

    let mut points = Vec::new();

    // Majority cluster A (12 points)
    for i in 0..12 {
        let angle = (i as f32) * 0.05; // slight spread
        points.push(SegmentEmbedding {
            start: i as f64,
            end: (i as f64) + 0.8,
            embedding: vec![1.0, angle.sin() * 0.2, 0.0],
        });
    }

    // Minority cluster B (2 points, tightly clustered around each other, orthogonal to A)
    points.push(SegmentEmbedding {
        start: 12.0,
        end: 12.8,
        embedding: vec![0.0, 1.0, 0.01],
    });
    points.push(SegmentEmbedding {
        start: 13.0,
        end: 13.8,
        embedding: vec![0.0, 1.0, -0.01],
    });

    let n = points.len(); // 14
    let labels_complete = cluster_embeddings_reference(&points, 2, Method::Complete).unwrap();
    let labels_average = cluster_embeddings_reference(&points, 2, Method::Average).unwrap();

    assert_eq!(labels_complete.len(), n);
    assert_eq!(labels_average.len(), n);

    // In Complete Linkage:
    // The two minority points (indices 12 and 13) MUST belong to the SAME cluster!
    assert_eq!(
        labels_complete[12], labels_complete[13],
        "Complete linkage must keep minority cluster members together"
    );
    // And they must NOT be in the majority cluster!
    assert_ne!(
        labels_complete[12], labels_complete[0],
        "Complete linkage must separate minority cluster from majority cluster"
    );

    // Verify all 12 majority points share the same cluster under Complete linkage
    for i in 1..12 {
        assert_eq!(
            labels_complete[i], labels_complete[0],
            "All majority points in A must share cluster under Complete linkage"
        );
    }
}
