use std::collections::HashMap;
use kodama::{linkage, Method};
use petgraph::unionfind::UnionFind;
use simsimd::SpatialSimilarity;

use super::SegmentEmbedding;

/// Cluster speaker embeddings using hierarchical agglomerative clustering.
///
/// Returns a Vec of speaker IDs (0-indexed), one per input segment, with at
/// most `max_speakers` distinct IDs.
pub fn cluster_embeddings(segment_embeddings: &[SegmentEmbedding], max_speakers: usize) -> Vec<usize> {
    let n = segment_embeddings.len();
    if n == 1 {
        return vec![0];
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

    let dendrogram = linkage(&mut condensed, n, Method::Complete);
    let steps = dendrogram.steps();
    let merges_to_make = n.saturating_sub(max_speakers);

    // kodama creates new cluster IDs n..2n-1 at each step.
    // By using a UnionFind of size 2*n, we can map both operands and the new ID 
    // into the same disjoint set. Petgraph handles the internal representative mapping.
    let mut uf = UnionFind::new(2 * n);

    for (step_idx, step) in steps.iter().enumerate().take(merges_to_make) {
        let new_cluster = n + step_idx;
        uf.union(step.cluster1, new_cluster);
        uf.union(step.cluster2, new_cluster);
    }

    let roots: Vec<usize> = (0..n).map(|i| uf.find(i)).collect();
    let mut label_map: HashMap<usize, usize> = HashMap::new();
    let mut next_label = 0usize;
    roots
        .iter()
        .map(|&root| {
            *label_map.entry(root).or_insert_with(|| {
                let l = next_label;
                next_label += 1;
                l
            })
        })
        .collect()
}

/// Cosine distance between two vectors: 1 - cos(a, b).
pub fn cosine_distance(a: &[f32], b: &[f32]) -> f64 {
    if a.is_empty() || b.is_empty() {
        return 1.0;
    }
    
    let is_zero_a = a.iter().all(|&x| x == 0.0);
    let is_zero_b = b.iter().all(|&x| x == 0.0);
    if is_zero_a || is_zero_b {
        return 1.0;
    }

    if a == b {
        return 0.0;
    }

    (f32::cosine(a, b).unwrap_or(1.0) as f64).clamp(0.0, 2.0)
}

#[cfg(test)]
mod tests {
    use super::*;


    #[test]
    fn cluster_embeddings_separates_speakers() {
        // Single segment
        let single = vec![SegmentEmbedding {
            start: 0.0,
            end: 1.0,
            embedding: vec![1.0, 0.0],
        }];
        assert_eq!(cluster_embeddings(&single, 2), vec![0]);

        // Four segments: two clusters
        let segments = vec![
            SegmentEmbedding {
                start: 0.0,
                end: 1.0,
                embedding: vec![1.0, 0.0, 0.0],
            },
            SegmentEmbedding {
                start: 1.0,
                end: 2.0,
                embedding: vec![0.95, 0.05, 0.0],
            },
            SegmentEmbedding {
                start: 2.0,
                end: 3.0,
                embedding: vec![0.0, 1.0, 0.0],
            },
            SegmentEmbedding {
                start: 3.0,
                end: 4.0,
                embedding: vec![0.0, 0.95, 0.05],
            },
        ];

        let labels = cluster_embeddings(&segments, 2);
        assert_eq!(labels.len(), 4);
        assert_eq!(labels[0], labels[1]);
        assert_eq!(labels[2], labels[3]);
        assert_ne!(labels[0], labels[2]);
    }
}
