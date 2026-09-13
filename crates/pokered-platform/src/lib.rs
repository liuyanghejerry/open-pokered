//! Small platform contracts shared below the game/data/rendering layers.
#![no_std]

#[cfg(not(target_os = "none"))]
extern crate std;

/// Synchronization primitives with the `std::sync` call surface used by the
/// game. Bare-metal implementations require a single-threaded executor and no
/// access from interrupt handlers.
pub mod sync {
    #[cfg(not(target_os = "none"))]
    pub use std::sync::{LazyLock, Mutex, MutexGuard, OnceLock};

    #[cfg(any(target_os = "none", test))]
    mod bare_metal {
        use core::cell::{Cell, UnsafeCell};
        use core::ops::{Deref, DerefMut};

        pub struct OnceLock<T> {
            value: UnsafeCell<Option<T>>,
            initializing: Cell<bool>,
        }

        // Safety: the platform contract excludes threads and interrupt access.
        unsafe impl<T> Sync for OnceLock<T> {}

        struct InitGuard<'a>(&'a Cell<bool>);
        impl Drop for InitGuard<'_> {
            fn drop(&mut self) {
                self.0.set(false);
            }
        }

        impl<T> OnceLock<T> {
            pub const fn new() -> Self {
                Self {
                    value: UnsafeCell::new(None),
                    initializing: Cell::new(false),
                }
            }

            pub fn get_or_init<F: FnOnce() -> T>(&self, init: F) -> &T {
                if self.get().is_none() {
                    assert!(
                        !self.initializing.replace(true),
                        "recursive OnceLock initialization"
                    );
                    let _guard = InitGuard(&self.initializing);
                    unsafe { *self.value.get() = Some(init()) };
                }
                self.get().expect("OnceLock initialized above")
            }

            pub fn get(&self) -> Option<&T> {
                unsafe { (*self.value.get()).as_ref() }
            }
        }

        pub struct LazyLock<T, F = fn() -> T> {
            cell: OnceLock<T>,
            init: F,
        }

        impl<T, F: Fn() -> T> LazyLock<T, F> {
            pub const fn new(init: F) -> Self {
                Self {
                    cell: OnceLock::new(),
                    init,
                }
            }
        }

        impl<T, F: Fn() -> T> Deref for LazyLock<T, F> {
            type Target = T;
            fn deref(&self) -> &T {
                self.cell.get_or_init(|| (self.init)())
            }
        }

        pub struct Mutex<T> {
            value: UnsafeCell<T>,
            locked: Cell<bool>,
        }

        // Safety: the platform contract excludes threads and interrupt access.
        unsafe impl<T> Sync for Mutex<T> {}

        pub struct MutexGuard<'a, T> {
            mutex: &'a Mutex<T>,
        }

        impl<T> Deref for MutexGuard<'_, T> {
            type Target = T;
            fn deref(&self) -> &T {
                unsafe { &*self.mutex.value.get() }
            }
        }

        impl<T> DerefMut for MutexGuard<'_, T> {
            fn deref_mut(&mut self) -> &mut T {
                unsafe { &mut *self.mutex.value.get() }
            }
        }

        impl<T> Drop for MutexGuard<'_, T> {
            fn drop(&mut self) {
                self.mutex.locked.set(false);
            }
        }

        impl<T> Mutex<T> {
            pub const fn new(value: T) -> Self {
                Self {
                    value: UnsafeCell::new(value),
                    locked: Cell::new(false),
                }
            }

            pub fn lock(&self) -> Result<MutexGuard<'_, T>, core::convert::Infallible> {
                assert!(
                    !self.locked.replace(true),
                    "recursive bare-metal mutex lock"
                );
                Ok(MutexGuard { mutex: self })
            }
        }

        pub struct LocalCell<T>(UnsafeCell<T>);

        // Safety: the platform contract excludes threads and interrupt access.
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
    pub use bare_metal::{LazyLock, LocalCell, Mutex, MutexGuard, OnceLock};

    #[cfg(test)]
    mod tests {
        use super::bare_metal::{Mutex, OnceLock};

        #[test]
        fn bare_mutex_rejects_aliasing_and_unlocks_on_drop() {
            let mutex = Mutex::new(1);
            let guard = mutex.lock().unwrap();
            let nested = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = mutex.lock();
            }));
            assert!(nested.is_err());
            drop(guard);
            *mutex.lock().unwrap() = 2;
            assert_eq!(*mutex.lock().unwrap(), 2);
        }

        #[test]
        fn bare_once_rejects_recursion_and_can_retry_after_unwind() {
            let cell = OnceLock::new();
            let recursive = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                cell.get_or_init(|| *cell.get_or_init(|| 1));
            }));
            assert!(recursive.is_err());
            assert_eq!(*cell.get_or_init(|| 2), 2);
        }
    }
}

/// Bare-metal substitute for the subset of `std::thread_local!` used by the
/// battle rule runtime. Each declaration becomes a single-threaded LocalCell.
#[cfg(target_os = "none")]
#[macro_export]
macro_rules! thread_local {
    ($(#[$attr:meta])* static $name:ident : $ty:ty = const { $init:expr } ; $($rest:tt)*) => {
        $(#[$attr])*
        static $name: $crate::sync::LocalCell<$ty> = $crate::sync::LocalCell::new($init);
        $crate::thread_local!($($rest)*);
    };
    ($(#[$attr:meta])* static $name:ident : $ty:ty = $init:expr ; $($rest:tt)*) => {
        $(#[$attr])*
        static $name: $crate::sync::LocalCell<$ty> = $crate::sync::LocalCell::new($init);
        $crate::thread_local!($($rest)*);
    };
    () => {};
}
