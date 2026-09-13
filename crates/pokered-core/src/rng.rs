//! Portable RNG facade for the game logic.
//!
//! Replaces direct `rand::random()` / `rand::thread_rng()` calls: those need
//! `rand`'s `std` feature (OS thread-locals + entropy), which does not exist
//! on the GBA build (`thumbv4t-none-eabi`, no_std). Hosted targets keep the
//! exact previous behavior (`rand::random` / `StdRng`); bare metal draws from
//! one global `SmallRng` seeded via `getrandom` (the custom source
//! registered by pokered-gba).

#[cfg(not(target_os = "none"))]
mod imp {
    pub use rand::rngs::StdRng as EntropyRng;
    pub use rand::random;
}

#[cfg(target_os = "none")]
mod imp {
    use rand::distributions::{Distribution, Standard};
    use rand::rngs::SmallRng;
    use rand::{RngCore, SeedableRng};

    // thumbv4t has no atomic operations at all (no AtomicBool/U8/U32), so
    // spin/once_cell mutexes cannot compile. The GBA build runs the game
    // loop single-threaded and no interrupt handler touches the RNG, so a
    // plain static is sound under that contract.
    static mut RNG: Option<SmallRng> = None;

    /// # Safety on hosted toolchains
    /// Only sound on single-threaded bare metal. Not compiled off-target.
    fn with_rng<T>(f: impl FnOnce(&mut SmallRng) -> T) -> T {
        unsafe {
            let slot = &mut *core::ptr::addr_of_mut!(RNG);
            let rng = slot.get_or_insert_with(SmallRng::from_entropy);
            f(rng)
        }
    }

    /// `rand::random` equivalent drawing from the global PRNG.
    pub fn random<T>() -> T
    where
        Standard: Distribution<T>,
    {
        with_rng(|rng| Standard.sample(rng))
    }

    pub type EntropyRng = SmallRng;
}

pub use imp::{random, EntropyRng};
