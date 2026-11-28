//! What the generator reports of each plant list it writes (beside the crates, never in the repository): its count and
//! hash, so that the report held in reports/ (compared by tests/written.rs) holds every plant list as written.

/// The report's line of the plant list `name` holding `text`: its plants and its FNV-1a hash.
pub fn reported(name: &str, text: &str) -> String {
    let hash = text.bytes().fold(FNV_OFFSET, |hash, byte| (hash ^ u64::from(byte)).wrapping_mul(FNV_PRIME));
    format!("PLANTS {name}: {} plants, FNV-1a {hash:016x}\n", text.matches("\nplant: ").count())
}

/// FNV-1a's 64-bit offset basis and prime.
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

#[cfg(test)]
mod tests {
    use super::reported;

    /// Each plant is counted, and the hash is FNV-1a's of the text (the empty text's is the offset basis; "a"'s is
    /// FNV-1a's published 0xaf63dc4c8601ec8c).
    #[test]
    fn counts_and_hashes() {
        assert_eq!(reported("none", ""), "PLANTS none: 0 plants, FNV-1a cbf29ce484222325\n");
        assert_eq!(reported("a", "a"), "PLANTS a: 0 plants, FNV-1a af63dc4c8601ec8c\n");
        assert!(reported("two", "head\nplant: one\nend\n\nplant: two\nend\n").starts_with("PLANTS two: 2 plants, FNV-1a "));
    }
}
