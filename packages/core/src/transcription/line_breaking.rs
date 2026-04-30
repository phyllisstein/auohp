use super::types::Word;

/// Max words per caption line.
const MAX_WIDTH: usize = 37;
/// Target words per second for each caption. BBC guidelines: 160--180WPM.
const TARGET_WPS: f64 = 2.7;
/// Additional badness penalty for captions that are too long to read.
const TEMPORAL_WEIGHT: f64 = 3.0;

enum Item {
    Box {
        width: usize,
        length: f64,
        word_index: usize,
    },
    Glue {
        width: usize,
        length: f64,
    },
    Penalty {
        cost: f64,
    },
}

#[derive(Debug)]
struct Breakpoint {
    item_index: usize,
    cumulative_width: usize,
    cumulative_length: f64,
}

#[derive(Debug, Clone)]
struct Caption {
    start_word: usize,
    end_word: usize,
    start_time: f64,
    end_time: f64,
    words: Vec<Word>,
}

fn find_breakpoints(candidate_breakpoints: &[Breakpoint]) -> Vec<usize> {
    // `breakpoints` contains every possible breakpoint, at every `Item::Glue`.
    // See line 36 in `collect_breakpoints`.
    let num_breakpoints = candidate_breakpoints.len();

    // The actual computed value of total badness up to the breakpoint in
    // `breakpoints` with a matching index. `min_cost[i]` is the lowest computed
    // badness for breakpoints up to `breakpoints[i]`.
    let mut min_cost = vec![f64::INFINITY; num_breakpoints];

    // The index in `breakpoints` of the "best" breakpoint preceding the
    // breakpoint at the index matching `best_predecessor`.
    // `best_predecessor[j]` is the index `i` in `breakpoints` such that
    // starting a caption at `breakpoints[i]` and ending it at `breakpoints[j]`
    // produces the lowest `min_cost[j]`. Breakpoint 10's best predecessor might
    // be breakpoint 3 if that produces a good caption spanning 7 breakpoints.
    let mut best_predecessor = vec![0usize; num_breakpoints];

    min_cost[0] = 0.0;

    for candidate in 1..num_breakpoints {
        for predecessor in (0..candidate).rev() {
            // Box characters between the two breakpoints, plus one space per
            // inter-word gap. There are (candidate - predecessor - 1) gaps
            // because N words have N-1 spaces between them.
            let candidate_caption_width = candidate_breakpoints[candidate].cumulative_width
                - candidate_breakpoints[predecessor].cumulative_width
                + (candidate - predecessor - 1);
            if candidate_caption_width > MAX_WIDTH {
                break;
            }

            let candidate_caption_length = candidate_breakpoints[candidate].cumulative_length
                - candidate_breakpoints[predecessor].cumulative_length;
            let candidate_caption_words = candidate - predecessor;
            let candidate_caption_wps = candidate_caption_words as f64 / candidate_caption_length;

            // Normalize length and width badness to roughly [0, 1].
            let width_badness =
                ((MAX_WIDTH as f64 - candidate_caption_width as f64) / MAX_WIDTH as f64).powi(2);
            let length_badness = ((candidate_caption_wps - TARGET_WPS) / TARGET_WPS).powi(2);

            let total_cost =
                min_cost[predecessor] + width_badness + length_badness * TEMPORAL_WEIGHT;

            if total_cost < min_cost[candidate] {
                min_cost[candidate] = total_cost;
                best_predecessor[candidate] = predecessor;
            }
        }
    }

    let mut break_sequence = vec![num_breakpoints - 1];
    let mut current = num_breakpoints - 1;
    while current != 0 {
        current = best_predecessor[current];
        break_sequence.push(current);
    }

    break_sequence.reverse();

    break_sequence
}

fn collect_candidate_breakpoints(items: &[Item]) -> Vec<Breakpoint> {
    let mut breakpoints: Vec<Breakpoint> = vec![Breakpoint {
        item_index: 0,
        cumulative_width: 0,
        cumulative_length: 0.0,
    }];
    let mut cumulative_width: usize = 0;
    let mut cumulative_length: f64 = 0.0;

    for (i, item) in items.iter().enumerate() {
        match item {
            Item::Glue { length, .. } => {
                breakpoints.push(Breakpoint {
                    item_index: i,
                    cumulative_width,
                    cumulative_length,
                });
                cumulative_length += length;
            }
            Item::Box { width, length, .. } => {
                cumulative_width += width;
                cumulative_length += length
            }
            _ => {}
        }
    }

    breakpoints.push(Breakpoint {
        item_index: items.len(),
        cumulative_width,
        cumulative_length,
    });

    breakpoints
}

fn build_items(words: &[Word]) -> Vec<Item> {
    let mut items: Vec<Item> = Vec::new();

    for (i, word) in words.iter().enumerate() {
        if i > 0 {
            items.push(Item::Glue {
                width: 1,
                length: word.start - words[i - 1].end,
            });
        }

        items.push(Item::Box {
            width: word.word.len(),
            length: word.end - word.start,
            word_index: i,
        });
    }

    items
}

fn build_captions(break_sequence: &[usize], words: &[Word]) -> Vec<Caption> {
    let mut captions: Vec<Caption> = Vec::new();

    for window in break_sequence.windows(2) {
        let start_word = window[0];
        let end_word = window[1];
        let subwords = &words[start_word..end_word];
        let start_time = subwords.first().unwrap().start;
        let end_time = subwords.last().unwrap().end;

        captions.push(Caption {
            start_word,
            end_word,
            start_time,
            end_time,
            words: subwords.into(),
        });
    }

    captions
}

pub fn break_into_captions(words: &[Word]) -> Vec<Caption> {
    let box_items = build_items(words);
    let candidate_breakpoints = collect_candidate_breakpoints(&box_items);
    let break_sequence = find_breakpoints(&candidate_breakpoints);

    build_captions(&break_sequence, words)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn to_words(text: &str, start_time: f64, end_time: f64) -> Vec<Word> {
        let mut words: Vec<Word> = Vec::new();
        let mut cumulative_time = start_time;
        let split_text: Vec<&str> = text.split_whitespace().collect();
        let word_time = (end_time - start_time) / split_text.len() as f64;

        for word in split_text.into_iter() {
            let end_time = cumulative_time + word_time;
            words.push(Word {
                word: String::from(word),
                start_time: cumulative_time,
                end_time,
            });

            cumulative_time = end_time;
        }

        words
    }

    #[test]
    fn short_sentence_fits_without_splitting() {
        let words = to_words("We did the Ashes Action in October", 6023.2, 6029.202);
        let captions = break_into_captions(&words);

        assert_eq!(captions.len(), 1);
    }

    #[test]
    fn long_sentence_splits_at_optimal_breakpoints() {
        let words = to_words(
            "That idea of a political funeral really hit a chord with Warren",
            6036.004,
            6040.865,
        );
        let captions = break_into_captions(&words);

        assert_eq!(captions.len(), 2);

        assert_eq!(captions.first().unwrap().start_word, 0);
        assert_eq!(captions.first().unwrap().end_word, 6);
        assert_eq!(captions.last().unwrap().start_word, 6);
        assert_eq!(captions.last().unwrap().end_word, 12);
    }

    #[test]
    fn no_words_dropped_across_captions() {
        let words = to_words(
            "That idea of a political funeral really hit a chord with Warren",
            6036.004,
            6040.865,
        );
        let captions = break_into_captions(&words);

        assert_eq!(captions.first().unwrap().words, &words[0..6]);
        assert_eq!(captions.last().unwrap().words, &words[6..12]);
    }

    #[test]
    fn one_word_produces_one_caption() {
        let words = to_words("Ashes", 0.0, 0.0);
        let captions = break_into_captions(&words);

        assert_eq!(captions.len(), 1);
    }

    #[test]
    #[ignore] // Needs hard-break temporal penalties to pass
    fn too_fast_words_split_captions() {
        let words = vec![
            // Spoken at a pace close to TARGET_WPS
            Word {
                word: "So".into(),
                start_time: 0.0,
                end_time: 0.2,
            },
            Word {
                word: "Your".into(),
                start_time: 0.4,
                end_time: 1.0,
            },
            Word {
                word: "whole".into(),
                start_time: 1.3,
                end_time: 2.7,
            },
            // Spoken far too quickly
            Word {
                word: "life".into(),
                start_time: 3.0,
                end_time: 3.1,
            },
            Word {
                word: "was".into(),
                start_time: 3.1,
                end_time: 3.2,
            },
            Word {
                word: "about".into(),
                start_time: 3.2,
                end_time: 3.3,
            },
            Word {
                word: "ACT".into(),
                start_time: 3.3,
                end_time: 3.4,
            },
            Word {
                word: "UP".into(),
                start_time: 3.4,
                end_time: 3.5,
            },
        ];

        let captions = break_into_captions(&words);

        assert_eq!(captions.len(), 2);
    }

    #[test]
    #[ignore] // Needs hard-break temporal penalties to pass
    fn too_slow_words_split_captions() {
        let words = vec![
            // Spoken at a pace close to TARGET_WPS
            Word {
                word: "So".into(),
                start_time: 0.0,
                end_time: 0.2,
            },
            Word {
                word: "Your".into(),
                start_time: 0.4,
                end_time: 1.0,
            },
            Word {
                word: "whole".into(),
                start_time: 1.3,
                end_time: 2.7,
            },
            // Spoken far too slowly
            Word {
                word: "life".into(),
                start_time: 3.0,
                end_time: 5.0,
            },
            Word {
                word: "was".into(),
                start_time: 5.0,
                end_time: 6.0,
            },
            Word {
                word: "about".into(),
                start_time: 6.0,
                end_time: 7.0,
            },
            Word {
                word: "ACT".into(),
                start_time: 7.0,
                end_time: 8.0,
            },
            Word {
                word: "UP".into(),
                start_time: 8.0,
                end_time: 9.0,
            },
        ];

        let captions = break_into_captions(&words);

        assert_eq!(captions.len(), 2);
    }
}
