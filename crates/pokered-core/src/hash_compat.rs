//! no_std hash-map aliases with an atomic-free hasher.
//!
//! `crate::hash_compat::HashMap` is unavailable on bare-metal targets
//! (thumbv4t / GBA), so the engine uses `hashbrown` instead. hashbrown's
//! default hasher (foldhash) relies on `core::sync::atomic::AtomicUsize`,
//! which the armv4t target does not provide, so this module supplies a
//! fixed-seed [FxHasher] instead. FxHash is not cryptographically secure —
//! irrelevant here: the maps key on trusted engine identifiers (tile ids,
//! palette ids, event-flag names), not user input.
//!
//! The aliases are API-compatible with `std::collections::{HashMap, HashSet}`
//! for the engine's usage (`new`, `entry`, `get`, `insert`, `remove`,
//! `iter`, `keys`, `values`, `retain`, `drain`, …). Determinism note: unlike
//! std's `RandomState`, the seed is fixed, so iteration order is stable
//! across runs — a property the save/serialization paths benefit from.

use core::hash::{BuildHasher, Hasher};

/// FxHash — the rustc hash. Small, fast, allocation- and atomic-free.
#[derive(Clone, Default)]
pub struct FxHasher {
    hash: u64,
}

const FX_K: u64 = 0x517c_c1b7_2722_0a95;

impl FxHasher {
    #[inline]
    fn add_to_hash(&mut self, i: u64) {
        self.hash = (self.hash.rotate_left(5) ^ i).wrapping_mul(FX_K);
    }
}

impl Hasher for FxHasher {
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.add_to_hash(*b as u64);
        }
    }

    #[inline]
    fn write_u8(&mut self, i: u8) {
        self.add_to_hash(i as u64);
    }

    #[inline]
    fn write_u16(&mut self, i: u16) {
        self.add_to_hash(i as u64);
    }

    #[inline]
    fn write_u32(&mut self, i: u32) {
        self.add_to_hash(i as u64);
    }

    #[inline]
    fn write_u64(&mut self, i: u64) {
        self.add_to_hash(i);
    }

    #[inline]
    fn write_usize(&mut self, i: usize) {
        self.add_to_hash(i as u64);
    }

    #[inline]
    fn write_u128(&mut self, i: u128) {
        self.add_to_hash(i as u64);
        self.add_to_hash((i >> 64) as u64);
    }

    #[inline]
    fn finish(&self) -> u64 {
        self.hash
    }
}

/// Const-constructible builder for [FxHasher].
///
/// A unit struct (rather than `BuildHasherDefault<FxHasher>`) so empty maps
/// can be declared as `static`s — `HashMap::with_hasher(FxBuildHasher)` is a
/// const expression, no lazy init or atomics needed.
#[derive(Clone, Copy, Default)]
pub struct FxBuildHasher;

impl BuildHasher for FxBuildHasher {
    type Hasher = FxHasher;

    #[inline]
    fn build_hasher(&self) -> FxHasher {
        FxHasher { hash: 0 }
    }
}

/// Drop-in replacement for `crate::hash_compat::HashMap`.
pub type HashMap<K, V> = hashbrown::HashMap<K, V, FxBuildHasher>;

/// Drop-in replacement for `std::collections::HashSet`.
pub type HashSet<T> = hashbrown::HashSet<T, FxBuildHasher>;
