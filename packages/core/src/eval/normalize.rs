//! Text normalisation: reduce truth and hypothesis to a comparable token stream.
//!
//! Both sides of every comparison pass through this function, so what matters is
//! not that any individual rule is "right" in the abstract but that it is applied
//! *identically* to both. A rule that mangles both streams the same way costs
//! nothing; a rule applied to only one side manufactures errors that aren't there.
//!
//! That principle is why the cleaned ground truth deliberately keeps casing,
//! contractions, and punctuation intact --- normalising it by hand at authoring
//! time and again here, by two slightly different sets of rules, is exactly how
//! the two streams end up disagreeing for reasons that have nothing to do with
//! the model.

/// Contractions expanded on both sides, so `"don't"` and `"do not"` compare equal.
///
/// Expansion (rather than contraction) is the right direction because it is
/// total: every contraction has one expansion, but `"do not"` could contract to
/// `"don't"` or stay put, and picking wrong splits a match into a substitution.
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
    // Deliberately no bare ("cause", "because") entry: these are substring
    // replacements over the whole string, so it would rewrite the "cause" inside
    // "because" and yield "bebecause".
    ("n't", " not"),
    ("'re", " are"),
    ("'ve", " have"),
    ("'ll", " will"),
    ("'d", " would"),
    ("'m", " am"),
];

/// Interjections a human transcriber silently drops but Whisper reports.
///
/// These are *not* removed by `normalize` --- they are removed by
/// [`strip_fillers`], which the scorer applies only when classifying insertions.
/// Dropping them from the WER stream outright would hide real hallucinations;
/// keeping them uncategorised would attribute the transcriber's editorial habits
/// to the model. The distinction is the point.
pub const FILLERS: &[&str] = &[
    "uh", "um", "er", "ah", "eh", "hmm", "mm", "mhm", "uh-huh", "mm-hmm",
];

/// Multi-word verbal tics that transcribers routinely delete.
///
/// These have to be matched as *phrases*, not word lists. Measured against this
/// corpus the largest insertion counts were `you`(21) and `know`(17) --- "you
/// know" being excised throughout --- but adding `you` and `know` to [`FILLERS`]
/// would misclassify every genuine use of two very ordinary words. The tic is the
/// sequence; the words are innocent.
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

    // Typographic folding. Curly quotes matter more than they look: the truth is
    // OCR'd from a PDF and uses U+2019 throughout, while Whisper emits ASCII "'".
    // Without this fold every single contraction in the file is a substitution.
    s = s
        .replace(['\u{2018}', '\u{2019}'], "'")
        .replace(['\u{201C}', '\u{201D}'], "\"")
        .replace(['\u{2013}', '\u{2014}'], " ")
        .replace('\u{2026}', " ");

    // Symbols that are *spoken as words*. Dropping them as punctuation loses a
    // real token on one side only: the poster is written "Silence = Death" but
    // said "silence equals death", so a faithful transcription scored as a
    // three-token phrase against a two-token truth misses the archive's single
    // most recognisable term.
    s = s
        .replace('=', " equals ")
        .replace('&', " and ")
        .replace('%', " percent ")
        .replace('+', " plus ");

    // Hyphens and slashes join words that either side may or may not have joined
    // ("leadership-obsessed" vs "leadership obsessed"). Splitting both is safe;
    // joining both is not, because only one side ever has the hyphen.
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

/// Pronouns that can legitimately precede a bare clitic.
///
/// Deliberately a closed list. Reattaching `d` or `s` after an arbitrary word
/// would corrupt ordinary text --- "the d train", "vitamin d" --- so only
/// pronouns, where the contraction reading is the overwhelmingly likely one, are
/// eligible.
const CLITIC_HOSTS: &[&str] = &["i", "you", "we", "they", "he", "she", "it", "that", "there", "who"];
const CLITICS: &[&str] = &["m", "re", "ve", "ll", "d", "s"];

/// Rejoin contraction clitics that lost their apostrophe.
///
/// Whisper intermittently emits `wouldn t` and `I m` where the truth has
/// `wouldn’t` and `I’m`. Left alone the two sides tokenise differently --- one
/// expands to `would not`, the other stays `wouldn` + `t` --- and every affected
/// contraction becomes a spurious substitution *plus* a spurious insertion.
///
/// That is not a cosmetic miscount: substitutions are the one bucket on this
/// fixture that reliably indicates a real model error, so noise here corrupts
/// the signal the whole campaign steers by. It showed up as `not -> t` and
/// `would -> wouldn` sitting at the top of the substitution taxonomy.
fn reattach_clitics(s: &str) -> String {
    let toks: Vec<&str> = s.split_whitespace().collect();
    let mut out: Vec<String> = Vec::with_capacity(toks.len());

    for tok in toks {
        let joined = match out.last() {
            // "wouldn t" -> "wouldn't". The host ending in `n` is what makes this
            // unambiguous; no ordinary word is followed by a bare `t`.
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
/// Returns multiple tokens when a numeral expands ("200" -> ["two", "hundred"]),
/// which is why the caller uses `flat_map`. Whisper writes digits; human
/// transcribers write words. Left alone, every number in the file is an error.
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
/// `digits` is the token's original digit count, which is what lets us tell a
/// year from a quantity: a bare 4-digit number in the 1100..=2099 range is read
/// the way people actually say years ("1985" -> "nineteen eighty five"), not as
/// a cardinal ("one thousand nine hundred eighty five"). Getting this wrong is
/// not a rounding error --- it turns every date in an oral history into four or
/// five spurious substitutions.
fn spell_number(n: u64, digits: usize) -> Vec<String> {
    if digits == 4 && (1100..=2099).contains(&n) {
        // Round thousands are said as thousands ("two thousand"), not as a
        // hundreds-pair ("twenty hundred"), so they fall through to `cardinal`.
        if n % 1_000 == 0 {
            return cardinal(n);
        }
        let (hi, lo) = (n / 100, n % 100);
        // 1900 -> "nineteen hundred".
        if lo == 0 {
            let mut v = cardinal(hi);
            v.push("hundred".into());
            return v;
        }
        let mut v = cardinal(hi);
        // "nineteen oh five", not "nineteen five".
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
        // The OCR'd truth uses U+2019; Whisper emits ASCII. These must agree.
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
        // "the d train" must not become "the'd train"; only pronouns host a
        // bare clitic, and only an n-final stem hosts a bare "t".
        assert_eq!(norm("the d train"), ["the", "d", "train"]);
        assert_eq!(norm("vitamin d deficiency"), ["vitamin", "d", "deficiency"]);
        assert_eq!(norm("a t shirt"), ["a", "t", "shirt"]);
    }

    #[test]
    fn reads_spoken_symbols_as_the_words_they_are() {
        // The poster is written "Silence = Death" and said "silence equals
        // death". Both spellings must reduce to the same tokens, or a correct
        // transcription is scored as a miss on the archive's best-known phrase.
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
