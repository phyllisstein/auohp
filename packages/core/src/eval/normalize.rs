//! Text normalisation: reduce truth and hypothesis to a comparable token stream.
//!
//! Both sides pass through the same rules; a rule applied to only one side
//! manufactures errors. That is why the cleaned truth keeps casing,
//! contractions, and punctuation intact rather than being pre-normalised by
//! hand.

/// Contractions expanded on both sides, so `"don't"` and `"do not"` compare equal.
///
/// Expansion is total (each contraction has one expansion); contracting is not.
const CONTRACTIONS: &[(&str, &str)] = &[
    ("can't", "can not"),
    ("cannot", "can not"),
    ("won't", "will not"),
    ("shan't", "shall not"),
    ("ain't", "is not"),
    ("let's", "let us"),
    ("y'all", "you all"),
    ("gonna", "going to"),
    ("wanna", "want to"),
    ("gotta", "got to"),
    ("'cause", "because"),
    // No bare ("cause", "because"): replacements are substring-based and would
    // turn "because" into "bebecause".
    ("n't", " not"),
    ("'re", " are"),
    ("'ve", " have"),
    ("'ll", " will"),
    ("'d", " would"),
    ("'m", " am"),
];

/// Interjections a human transcriber silently drops but Whisper reports.
///
/// Not removed by `normalize`; [`strip_fillers`] applies only when classifying
/// insertions. Dropping them from the WER stream would hide real hallucinations.
pub const FILLERS: &[&str] = &[
    "uh", "um", "er", "ah", "eh", "hmm", "mm", "mhm", "uh-huh", "mm-hmm",
];

/// Multi-word verbal tics that transcribers routinely delete.
///
/// Matched as phrases: adding `you` and `know` to [`FILLERS`] would misclassify
/// every ordinary use of those words.
pub const FILLER_PHRASES: &[&[&str]] = &[
    &["you", "know"],
    &["i", "mean"],
    &["sort", "of"],
    &["kind", "of"],
    &["or", "whatever"],
    &["and", "so", "on"],
    &["you", "see"],
    &["i", "guess"],
];

/// Lowercase, fold typography, expand contractions, spell out numerals, strip
/// punctuation, and split on whitespace.
pub fn normalize(text: &str) -> Vec<String> {
    let mut s = text.to_lowercase();

    // The OCR'd truth uses U+2019; Whisper emits ASCII "'". Unfolded, every
    // contraction is a substitution.
    s = s
        .replace(['\u{2018}', '\u{2019}'], "'")
        .replace(['\u{201C}', '\u{201D}'], "\"")
        .replace(['\u{2013}', '\u{2014}'], " ")
        .replace('\u{2026}', " ");

    // Symbols spoken as words: "Silence = Death" is said "silence equals death".
    s = s
        .replace('=', " equals ")
        .replace('&', " and ")
        .replace('%', " percent ")
        .replace('+', " plus ");

    // Split rather than join: only one side may have the hyphen.
    s = s.replace(['-', '/', '\u{2010}', '\u{2011}'], " ");

    s = reattach_clitics(&s);

    for (from, to) in CONTRACTIONS {
        if s.contains(from) {
            s = s.replace(from, to);
        }
    }

    s.split_whitespace()
        .flat_map(|tok| {
            let cleaned: String = tok
                .chars()
                .filter(|c| c.is_alphanumeric() || *c == '\'')
                .collect();
            let cleaned = cleaned.trim_matches('\'').to_string();
            expand_token(&cleaned)
        })
        .filter(|t| !t.is_empty())
        .collect()
}

/// Pronouns that can precede a bare clitic. Closed so "the d train" and
/// "vitamin d" are not rejoined.
const CLITIC_HOSTS: &[&str] = &["i", "you", "we", "they", "he", "she", "it", "that", "there", "who"];
const CLITICS: &[&str] = &["m", "re", "ve", "ll", "d", "s"];

/// Rejoin contraction clitics that lost their apostrophe.
///
/// Whisper sometimes emits `wouldn t` and `I m` where the truth has `wouldn’t`
/// and `I’m`; unrejoined, each becomes a spurious substitution plus insertion,
/// polluting the substitution taxonomy.
fn reattach_clitics(s: &str) -> String {
    let toks: Vec<&str> = s.split_whitespace().collect();
    let mut out: Vec<String> = Vec::with_capacity(toks.len());

    for tok in toks {
        let joined = match out.last() {
            // An `n`-final host makes a bare `t` unambiguous.
            Some(prev) if tok == "t" && prev.ends_with('n') && prev.len() > 2 => true,
            Some(prev) if CLITICS.contains(&tok) && CLITIC_HOSTS.contains(&prev.as_str()) => true,
            _ => false,
        };
        if joined {
            let prev = out.last_mut().expect("checked above");
            prev.push('\'');
            prev.push_str(tok);
        } else {
            out.push(tok.to_string());
        }
    }
    out.join(" ")
}

/// Remove filler words from a token stream. Used for classifying insertions, not
/// for computing WER.
pub fn strip_fillers(tokens: &[String]) -> Vec<String> {
    tokens
        .iter()
        .filter(|t| !FILLERS.contains(&t.as_str()))
        .cloned()
        .collect()
}

/// True if a token is a filler a human transcriber would routinely omit.
pub fn is_filler(token: &str) -> bool {
    FILLERS.contains(&token)
}

/// Expand a single cleaned token, spelling any numeral out into words.
///
/// Whisper writes digits; the truth writes words. One numeral can expand to
/// several tokens ("200" -> ["two", "hundred"]).
fn expand_token(tok: &str) -> Vec<String> {
    if tok.is_empty() {
        return vec![];
    }
    if !tok.chars().all(|c| c.is_ascii_digit()) {
        return vec![tok.to_string()];
    }
    match tok.parse::<u64>() {
        Ok(n) => spell_number(n, tok.len()),
        Err(_) => vec![tok.to_string()],
    }
}

const ONES: [&str; 20] = [
    "zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten",
    "eleven", "twelve", "thirteen", "fourteen", "fifteen", "sixteen", "seventeen", "eighteen",
    "nineteen",
];
const TENS: [&str; 10] = [
    "", "", "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety",
];

/// Spell `n` as English words.
///
/// A bare 4-digit number in 1100..=2099 is read as a year ("1985" -> "nineteen
/// eighty five"), not a cardinal; `digits` is the original digit count.
fn spell_number(n: u64, digits: usize) -> Vec<String> {
    if digits == 4 && (1100..=2099).contains(&n) {
        // "two thousand", not "twenty hundred".
        if n % 1_000 == 0 {
            return cardinal(n);
        }
        let (hi, lo) = (n / 100, n % 100);
        if lo == 0 {
            let mut v = cardinal(hi);
            v.push("hundred".into());
            return v;
        }
        let mut v = cardinal(hi);
        // "nineteen oh five".
        if lo < 10 {
            v.push("oh".into());
        }
        v.extend(cardinal(lo));
        return v;
    }
    cardinal(n)
}

fn cardinal(n: u64) -> Vec<String> {
    if n < 20 {
        return vec![ONES[n as usize].to_string()];
    }
    if n < 100 {
        let mut v = vec![TENS[(n / 10) as usize].to_string()];
        if n % 10 != 0 {
            v.push(ONES[(n % 10) as usize].to_string());
        }
        return v;
    }
    if n < 1_000 {
        let mut v = cardinal(n / 100);
        v.push("hundred".into());
        if n % 100 != 0 {
            v.extend(cardinal(n % 100));
        }
        return v;
    }
    if n < 1_000_000 {
        let mut v = cardinal(n / 1_000);
        v.push("thousand".into());
        if n % 1_000 != 0 {
            v.extend(cardinal(n % 1_000));
        }
        return v;
    }
    let mut v = cardinal(n / 1_000_000);
    v.push("million".into());
    if n % 1_000_000 != 0 {
        v.extend(cardinal(n % 1_000_000));
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    fn norm(s: &str) -> Vec<String> {
        normalize(s)
    }

    #[test]
    fn lowercases_and_strips_punctuation() {
        assert_eq!(norm("Well, I think that Larry!"), ["well", "i", "think", "that", "larry"]);
    }

    #[test]
    fn folds_curly_quotes_so_contractions_match_across_sources() {
        assert_eq!(norm("didn\u{2019}t"), norm("didn't"));
        assert_eq!(norm("didn't"), ["did", "not"]);
    }

    #[test]
    fn expands_contractions_consistently() {
        assert_eq!(norm("don't"), norm("do not"));
        assert_eq!(norm("we're"), norm("we are"));
        assert_eq!(norm("can't"), norm("cannot"));
    }

    #[test]
    fn splits_hyphenated_compounds() {
        assert_eq!(norm("leadership-obsessed"), norm("leadership obsessed"));
    }

    #[test]
    fn spells_cardinals() {
        assert_eq!(norm("200 people"), ["two", "hundred", "people"]);
        assert_eq!(norm("25"), ["twenty", "five"]);
        assert_eq!(norm("0"), ["zero"]);
    }

    #[test]
    fn reads_four_digit_years_the_way_people_say_them() {
        assert_eq!(norm("1985"), ["nineteen", "eighty", "five"]);
        assert_eq!(norm("1905"), ["nineteen", "oh", "five"]);
        // Not a year: spelled as a quantity.
        assert_eq!(norm("5000"), ["five", "thousand"]);
    }

    #[test]
    fn digits_and_words_converge() {
        assert_eq!(norm("two hundred people"), norm("200 people"));
    }

    #[test]
    fn reattaches_contractions_whisper_emitted_without_apostrophes() {
        // Observed verbatim in 000-reencode: "because I wouldn t say that I m going".
        assert_eq!(norm("I wouldn t say"), norm("I wouldn't say"));
        assert_eq!(norm("I wouldn t say"), ["i", "would", "not", "say"]);
        assert_eq!(norm("that I m going"), norm("that I'm going"));
        assert_eq!(norm("you re here"), norm("you're here"));
        assert_eq!(norm("they ve gone"), norm("they've gone"));
    }

    #[test]
    fn does_not_reattach_clitics_to_arbitrary_words() {
        assert_eq!(norm("the d train"), ["the", "d", "train"]);
        assert_eq!(norm("vitamin d deficiency"), ["vitamin", "d", "deficiency"]);
        assert_eq!(norm("a t shirt"), ["a", "t", "shirt"]);
    }

    #[test]
    fn reads_spoken_symbols_as_the_words_they_are() {
        assert_eq!(norm("Silence = Death"), ["silence", "equals", "death"]);
        assert_eq!(norm("Silence = Death"), norm("Silence equals Death"));
        assert_eq!(norm("R&D"), ["r", "and", "d"]);
    }

    #[test]
    fn fillers_are_kept_in_the_stream_but_identifiable() {
        let t = norm("um I think");
        assert_eq!(t, ["um", "i", "think"]);
        assert!(is_filler(&t[0]));
        assert_eq!(strip_fillers(&t), ["i", "think"]);
    }
}
