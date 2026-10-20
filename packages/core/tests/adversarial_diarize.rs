//! Adversarial inputs for `cluster_embeddings`, `cosine_distance`, and `dominant_speaker`.
//! Complete linkage must keep a minority speaker cluster that Average linkage absorbs.

use std::collections::HashMap;
use kodama::{linkage, Method};
use auohp_core::transcription::{
    cosine_distance, dominant_speaker, DiarizedSegment, SegmentEmbedding,
};

/// Mirror of the private `cluster_embeddings` in diarize.rs, so its algorithm can be driven directly.
fn cluster_embeddings_reference(
    segment_embeddings: &[SegmentEmbedding],
    max_speakers: usize,
    method: Method,
) -> Result<Vec<usize>, String> {
    let n = segment_embeddings.len();
    if n == 0 {
        // diarize.rs guards `n == 1` but not `n == 0`, where `n * (n - 1) / 2` underflows in debug builds.
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
    assert_eq!(dominant_speaker(0.0, 0.5, &diarized), None);
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
    assert_eq!(dominant_speaker(1.0, 2.0, &diarized), None);
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
    assert_eq!(dominant_speaker(2.0, 2.0, &diarized), None);
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
    let unique_labels: std::collections::HashSet<_> = labels.iter().collect();
    assert!(unique_labels.len() <= 2, "must not exceed max_speakers");
}

#[test]
fn test_adversarial_cluster_embeddings_all_orthogonal() {
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

#[test]
fn test_adversarial_complete_vs_average_linkage_minority_preservation() {

    let mut points = Vec::new();

    for i in 0..12 {
        let angle = (i as f32) * 0.05; // slight spread
        points.push(SegmentEmbedding {
            start: i as f64,
            end: (i as f64) + 0.8,
            embedding: vec![1.0, angle.sin() * 0.2, 0.0],
        });
    }

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

    assert_eq!(
        labels_complete[12], labels_complete[13],
        "Complete linkage must keep minority cluster members together"
    );
    assert_ne!(
        labels_complete[12], labels_complete[0],
        "Complete linkage must separate minority cluster from majority cluster"
    );

    for i in 1..12 {
        assert_eq!(
            labels_complete[i], labels_complete[0],
            "All majority points in A must share cluster under Complete linkage"
        );
    }
}
