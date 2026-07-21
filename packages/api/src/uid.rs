const SAFE: [char; 57] = [
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'j',
    'k', 'm', 'n', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z', 'A', 'B', 'C', 'D', 'E',
    'F', 'G', 'H', 'J', 'K', 'L', 'M', 'N', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z',
];

/// Generate a 12-character nanoid, omitting punctuation marks (even though they
/// would look cool) and Crockford-ambigous characters. Fixed to 12 characters
/// in length, safe for around a billion IDs.
pub fn generate() -> String {
    nanoid::nanoid!(12, &SAFE)
}

/// Generate an ID that won't be random enough to safely avoid collisions in a
/// global ID space, but could apply to elements of subtrees.
pub fn mini() -> String {
    nanoid::nanoid!(4)
}
