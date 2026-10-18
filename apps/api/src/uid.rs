const SAFE: [char; 59] = [
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'j',
    'k', 'm', 'n', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z', 'A', 'B', 'C', 'D', 'E',
    'F', 'G', 'H', 'J', 'K', 'L', 'M', 'N', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z',
    '.', '~',
];

/// Generate a 12-character nanoid, omitting Crockford-ambigous characters.
/// Fixed to 12 characters in length, safe for around a billion IDs.
pub fn generate() -> String {
    loop {
        let nid = nanoid::nanoid!(12, &SAFE);

        if !nid.starts_with(&['.', '~']) && !nid.ends_with(&['.', '~']) {
            return nid;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use asserting::prelude::*;

    #[test]
    fn inner_punctution_only() {
        let u = generate();
        assert_that!(u).does_not_start_with('.');
        let u = generate();
        assert_that!(u).does_not_start_with('~');

        let u = generate();
        assert_that!(u).does_not_end_with('.');
        let u = generate();
        assert_that!(u).does_not_end_with('~');
    }
}
