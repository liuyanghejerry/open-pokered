//! Fixed-seed word hashing for trusted ROM identifiers on 32-bit hardware.
//! Mixing a u64 for every byte otherwise calls software multiplication on
//! ARM7TDMI throughout map-script registration and lookups.
use core::hash::{BuildHasher, Hasher};

#[derive(Clone, Default)]
pub struct WordHasher(u32);

impl WordHasher {
    #[inline]
    fn mix(&mut self, word: u32) {
        self.0 = (self.0.rotate_left(5) ^ word).wrapping_mul(0x9e37_79b9);
    }
}

impl Hasher for WordHasher {
    #[inline]
    fn write(&mut self, mut bytes: &[u8]) {
        while let Some((word, tail)) = bytes.split_first_chunk::<4>() {
            self.mix(u32::from_le_bytes(*word));
            bytes = tail;
        }
        for &byte in bytes {
            self.mix(u32::from(byte));
        }
    }

    #[inline]
    fn write_u32(&mut self, word: u32) {
        self.mix(word);
    }

    #[inline]
    fn finish(&self) -> u64 {
        // Hashbrown uses the high bits as a control-byte fingerprint. Keep
        // those populated on both hosted tests and the 32-bit target.
        u64::from(self.0) | (u64::from(self.0) << 32)
    }
}

#[derive(Clone, Copy, Default)]
pub struct WordBuildHasher;

impl BuildHasher for WordBuildHasher {
    type Hasher = WordHasher;
    #[inline]
    fn build_hasher(&self) -> Self::Hasher {
        WordHasher::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::hash::Hash;

    #[test]
    fn words_tails_and_upper_integer_bits_affect_hashes() {
        let hash = |value: &str| WordBuildHasher.hash_one(value);
        for left in ["", "a", "ab", "abc", "abcd", "abcde", "storyline_talkOak1"] {
            assert_eq!(hash(left), hash(left));
            assert_ne!(hash(left), hash("storyline_talkOak2"));
        }
        assert_ne!(
            WordBuildHasher.hash_one(1u64),
            WordBuildHasher.hash_one(1u64 << 32)
        );
        let mut a = WordHasher::default();
        "ab".hash(&mut a);
        "c".hash(&mut a);
        let mut b = WordHasher::default();
        "a".hash(&mut b);
        "bc".hash(&mut b);
        assert_ne!(a.finish(), b.finish());
        assert_ne!(hash("ViridianCity") >> 57, 0);
    }
}
