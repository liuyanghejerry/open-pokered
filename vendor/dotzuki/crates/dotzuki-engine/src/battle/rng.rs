//! Injected randomness for the battle turn-execution driver (P0b).
//!
//! The engine is **100% game-agnostic** and must never link the `rand` crate
//! (architecture rule C2). All randomness used by [`crate::battle::driver`]
//! flows through the [`BattleRng`] trait, so the *game* owns the generator and
//! therefore the exact draw sequence.
//!
//! Controlling the draw order game-side is essential to reproduce Gen-1 quirks
//! such as the 1/256 "miss", critical-hit rolls, the speed-tie coin flip, and
//! partial-trap duration rolls: the original game consumes its RNG stream in a
//! specific order, and only the game knows that order.
//!
//! ## Determinism
//!
//! Implementations may wrap any generator (an LCG matching the original ROM, a
//! seedable PRNG for tests, or a fixed script of bytes). The driver only calls
//! the methods on this trait; it makes no assumptions about the distribution
//! beyond the documented contracts below.

/// A source of randomness for the battle driver.
///
/// The single required method is [`BattleRng::next_u8`]; the others have
/// default implementations layered on top of it so most games only implement
/// one method. Implementations are free to override any of them to match an
/// exact original-game draw sequence.

pub trait BattleRng {
    /// Return the next raw byte in the stream (`0..=255`).
    ///
    /// This is the lowest-level primitive and the one most faithful to the
    /// original 8-bit hardware RNG. Higher-level helpers derive from it.
    fn next_u8(&mut self) -> u8;

    /// Uniform integer in `[0, bound)`.
    ///
    /// Returns `0` when `bound == 0`. The default implementation folds
    /// successive bytes; games that need the *exact* original modulo behaviour
    /// (including its bias) should override this.
    fn range(&mut self, bound: u32) -> u32 {
        if bound == 0 {
            return 0;
        }
        if bound <= 256 {
            // Single byte covers the range; plain modulo mirrors the 8-bit
            // hardware path (and its bias) used by the original game.
            return (self.next_u8() as u32) % bound;
        }
        // Wider ranges: assemble enough bytes, then reduce.
        let mut acc: u32 = 0;
        let mut produced: u64 = 1;
        while produced < bound as u64 {
            acc = acc.wrapping_shl(8) | self.next_u8() as u32;
            produced = produced.saturating_mul(256);
        }
        acc % bound
    }

    /// Coin flip succeeding with probability `num / den`.
    ///
    /// Returns `false` when `den == 0`. Used for crit checks, the speed-tie
    /// flip, the 255/256-style hit checks, status-proc rolls, etc.
    fn chance(&mut self, num: u32, den: u32) -> bool {
        if den == 0 {
            return false;
        }
        self.range(den) < num
    }
}

/// A [`BattleRng`] that replays a fixed script of bytes, then repeats the last
/// byte (or `0` if empty) forever.
///
/// This makes driver behaviour fully deterministic in tests: a test can pin the
/// exact bytes the driver will observe and assert on the resulting event
/// stream, proving turn-order ties, gates, and residuals resolve as expected.
#[derive(Debug, Clone)]
pub struct ScriptedRng {
    bytes: Vec<u8>,
    pos: usize,
}

impl ScriptedRng {
    /// Create a scripted RNG that yields `bytes` in order.
    pub fn new(bytes: impl Into<Vec<u8>>) -> Self {
        Self {
            bytes: bytes.into(),
            pos: 0,
        }
    }

    /// Number of bytes consumed so far. Useful for asserting draw-order parity.
    pub fn consumed(&self) -> usize {
        self.pos
    }
}

impl BattleRng for ScriptedRng {
    fn next_u8(&mut self) -> u8 {
        let b = if self.bytes.is_empty() {
            0
        } else if self.pos < self.bytes.len() {
            self.bytes[self.pos]
        } else {
            *self.bytes.last().unwrap()
        };
        self.pos += 1;
        b
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scripted_yields_bytes_in_order_then_repeats_last() {
        let mut rng = ScriptedRng::new(vec![1, 2, 3]);
        assert_eq!(rng.next_u8(), 1);
        assert_eq!(rng.next_u8(), 2);
        assert_eq!(rng.next_u8(), 3);
        // Past the end → repeats the last byte.
        assert_eq!(rng.next_u8(), 3);
        assert_eq!(rng.consumed(), 4);
    }

    #[test]
    fn empty_script_yields_zero() {
        let mut rng = ScriptedRng::new(Vec::new());
        assert_eq!(rng.next_u8(), 0);
        assert_eq!(rng.next_u8(), 0);
    }

    #[test]
    fn range_zero_bound_is_zero() {
        let mut rng = ScriptedRng::new(vec![200]);
        assert_eq!(rng.range(0), 0);
    }

    #[test]
    fn range_small_bound_is_modulo_of_byte() {
        let mut rng = ScriptedRng::new(vec![10]);
        // 10 % 4 == 2
        assert_eq!(rng.range(4), 2);
    }

    #[test]
    fn range_wide_bound_assembles_bytes() {
        let mut rng = ScriptedRng::new(vec![0x00, 0x01]);
        // bound > 256 → two bytes assembled: (0x00 << 8 | 0x01) = 1; 1 % 1000 = 1
        assert_eq!(rng.range(1000), 1);
    }

    #[test]
    fn chance_uses_range() {
        // byte 0 → range(2) == 0 < 1 → true
        let mut rng = ScriptedRng::new(vec![0]);
        assert!(rng.chance(1, 2));
        // byte 1 → range(2) == 1, not < 1 → false
        let mut rng = ScriptedRng::new(vec![1]);
        assert!(!rng.chance(1, 2));
    }

    #[test]
    fn chance_zero_den_is_false() {
        let mut rng = ScriptedRng::new(vec![0]);
        assert!(!rng.chance(1, 0));
    }
}
