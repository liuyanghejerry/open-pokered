//! Lazy-init / mutex shims shared by the data registries.
//!
//! Hosted builds use std's `OnceLock` / `LazyLock` / `Mutex` unchanged. On
//! bare metal (GBA, `target_os = "none"`) the same names are backed by
//! single-threaded UnsafeCell state (thumbv4t has no atomics, so spin-based
//! shims cannot compile there).

#[cfg(not(target_os = "none"))]
pub use std::sync::{LazyLock, Mutex, MutexGuard, OnceLock};

mod bare_metal {
    // thumbv4t (GBA) has NO atomic operations — spin/once_cell cannot
    // compile. The GBA build runs the game loop single-threaded and no
    // interrupt handler touches these, so plain UnsafeCell state is sound
    // under that contract (same approach as pokered-core's rng module).
    use core::cell::UnsafeCell;
    use core::ops::{Deref, DerefMut};

    /// `std::sync::OnceLock` stand-in.
    pub struct OnceLock<T>(UnsafeCell<Option<T>>);

    // Sound on single-threaded bare metal.
    unsafe impl<T> Sync for OnceLock<T> {}

    impl<T> OnceLock<T> {
        pub const fn new() -> Self {
            Self(UnsafeCell::new(None))
        }

        pub fn get_or_init<F: FnOnce() -> T>(&self, f: F) -> &T {
            unsafe {
                let slot = &mut *self.0.get();
                slot.get_or_insert_with(f)
            }
        }

        pub fn get(&self) -> Option<&T> {
            unsafe { (*self.0.get()).as_ref() }
        }
    }

    /// `std::sync::LazyLock` stand-in.
    pub struct LazyLock<T, F = fn() -> T> {
        cell: OnceLock<T>,
        init: F,
    }

    impl<T, F: Fn() -> T> LazyLock<T, F> {
        pub const fn new(f: F) -> Self {
            Self {
                cell: OnceLock::new(),
                init: f,
            }
        }
    }

    impl<T, F: Fn() -> T> Deref for LazyLock<T, F> {
        type Target = T;
        fn deref(&self) -> &T {
            self.cell.get_or_init(|| (self.init)())
        }
    }

    /// `std::sync::Mutex` stand-in; `lock()` returns `Ok(guard)` so the
    /// existing `.lock().unwrap()` call sites stay valid.
    pub struct Mutex<T>(UnsafeCell<T>);

    unsafe impl<T> Sync for Mutex<T> {}

    pub struct MutexGuard<'a, T>(&'a mut T);

    impl<T> Deref for MutexGuard<'_, T> {
        type Target = T;
        fn deref(&self) -> &T {
            self.0
        }
    }

    impl<T> DerefMut for MutexGuard<'_, T> {
        fn deref_mut(&mut self) -> &mut T {
            self.0
        }
    }

    impl<T> Mutex<T> {
        pub const fn new(value: T) -> Self {
            Self(UnsafeCell::new(value))
        }

        pub fn lock(&self) -> Result<MutexGuard<'_, T>, core::convert::Infallible> {
            Ok(MutexGuard(unsafe { &mut *self.0.get() }))
        }
    }
}

#[cfg(target_os = "none")]
pub use bare_metal::{LazyLock, Mutex, MutexGuard, OnceLock};

// `thread_local!` is std-only. The GBA is single-threaded, so on bare metal
// the shim collapses thread-locals to plain statics with identical
// `.with(|x| x.borrow()/borrow_mut())` call surface.
#[cfg(target_os = "none")]
mod bare_metal_tls {
    use core::cell::UnsafeCell;

    /// Mirrors `thread_local!` semantics: the static's own type decides
    /// mutability (`RefCell<T>` / `Cell<T>` for mutating slots, plain values
    /// otherwise). `with` hands out `&T`.
    pub struct LocalCell<T>(UnsafeCell<T>);

    // Sound on the single-threaded GBA: no other thread can alias it.
    unsafe impl<T> Sync for LocalCell<T> {}

    impl<T> LocalCell<T> {
        pub const fn new(value: T) -> Self {
            Self(UnsafeCell::new(value))
        }

        pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
            f(unsafe { &*self.0.get() })
        }
    }
}

#[cfg(target_os = "none")]
pub(crate) use bare_metal_tls::LocalCell;

/// Bare-metal `thread_local!` stand-in: `thread_local! { static X: T = init; }`
/// becomes `static X: LocalCell<T>`; `X.with(|x: &T| …)` keeps working.
/// Bare-metal `thread_local!` stand-in. Handles the std surface actually used
/// in this crate: optional doc/attr comments, `= expr` / `= const { expr }`
/// initializers, and multiple `static` entries per block. Each static becomes
/// a `LocalCell` (plain static — the GBA game loop is single-threaded).
#[cfg(target_os = "none")]
#[macro_export]
macro_rules! thread_local {
    ($(#[$attr:meta])* static $name:ident : $ty:ty = const { $init:expr } ; $($rest:tt)*) => {
        $(#[$attr])*
        static $name: $crate::sync_compat::LocalCell<$ty> =
            $crate::sync_compat::LocalCell::new($init);
        $crate::thread_local!($($rest)*);
    };
    ($(#[$attr:meta])* static $name:ident : $ty:ty = $init:expr ; $($rest:tt)*) => {
        $(#[$attr])*
        static $name: $crate::sync_compat::LocalCell<$ty> =
            $crate::sync_compat::LocalCell::new($init);
        $crate::thread_local!($($rest)*);
    };
    () => {};
}
