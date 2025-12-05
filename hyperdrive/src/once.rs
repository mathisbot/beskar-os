//! A wrapper around lazily-initialized data.
//!
//! This structure is somewhat similar to `std::sync::Once`, and it does not provide interior mutability.
//! It is used to perform a one-time initialization of a value, and then provide a reference to it.
//!
//! If initialization fails, the value will be marked as poisoned and a panic will occur.
//! This behavior depends on panic unwinding, so it does not work in `no_std` environments
//! with `panic = "abort"`.
//!
//! If you need one-time initialization with interior mutability, consider combining this structure with a lock.
//!
//! ## Examples
//!
//! `Once` can obviously be used to perform one-time initialization:
//!
//! ```rust
//! # use hyperdrive::once::Once;
//! #
//! static ONCE: Once<u8> = Once::uninit();
//! assert!(ONCE.get().is_none());
//!
//! ONCE.call_once(|| 42);
//!
//! let value = ONCE.get().unwrap();
//! assert_eq!(*value, 42);
//! ```
//!
//! But it can also be used as a trick to ensure that an operation is only performed once:
//!
//! ```rust
//! # use hyperdrive::once::Once;
//! #
//! fn perform_once() {
//!     static PERFORM_ONCE: Once<()> = Once::uninit();
//!
//!     PERFORM_ONCE.call_once(|| {
//!         // Perform the operation
//!     });
//! }
//!
//! // This could be called on many threads,
//! // but the operation will only be performed once.
//! perform_once();
//! ```
//!
//! which can be written more concisely using the `call_once!` macro:
//!
//! ```rust
//! # use hyperdrive::once::call_once;
//! #
//! // This could be called on many threads,
//! // but the operation will only be performed once.
//! call_once!({
//!     // Perform the operation
//! });
//! ```
//!
//! Note that, in this case, if every thread calls the function `perform_once` at the same time,
//! non-executing threads won't block on the operation.
//! If you want to ensure the operation is complete before proceeding, use `Once::get`.
use core::cell::UnsafeCell;
use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicU8, Ordering};

#[macro_export]
/// A macro to simplify the usage of `Once` for one-time calls.
macro_rules! call_once {
    ($x:expr) => {{
        static CALL_ONCE: $crate::once::Once<()> = $crate::once::Once::uninit();
        CALL_ONCE.call_once(|| $x);
    }};
}
pub use call_once;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Possible states of the `Once` structure.
enum State {
    Initialized = 0,
    Initializing,
    Uninitialized,
    Poisoned,
}

impl State {
    #[must_use]
    #[inline]
    /// Converts the state to a `u8` value.
    pub const fn as_u8(self) -> u8 {
        match self {
            Self::Initialized => 0,
            Self::Initializing => 1,
            Self::Uninitialized => 2,
            Self::Poisoned => 3,
        }
    }

    #[must_use]
    #[inline]
    /// Converts a `u8` value to a `State`.
    /// Returns `None` if the value is not a valid state.
    pub const fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Initialized),
            1 => Some(Self::Initializing),
            2 => Some(Self::Uninitialized),
            3 => Some(Self::Poisoned),
            _ => None,
        }
    }

    #[must_use]
    #[inline]
    /// Converts a `u8` value to a `State`.
    ///
    /// # Safety
    ///
    /// The value must be a valid state.
    pub const unsafe fn from_u8_unchecked(value: u8) -> Self {
        unsafe { Self::from_u8(value).unwrap_unchecked() }
    }
}

/// Wrapper around `AtomicU8` to provide a more convenient API.
struct AtomicState(AtomicU8);

impl AtomicState {
    #[must_use]
    #[inline]
    pub const fn uninit() -> Self {
        Self::new(State::Uninitialized)
    }

    #[must_use]
    #[inline]
    pub const fn new(state: State) -> Self {
        Self(AtomicU8::new(state.as_u8()))
    }

    #[must_use]
    #[inline]
    pub fn load(&self, order: Ordering) -> State {
        let raw = self.0.load(order);
        // Safety: `AtomicState`'s API only allows storing valid states.
        // Thus, we can safely convert the raw value to a `State`.
        unsafe { State::from_u8_unchecked(raw) }
    }

    #[inline]
    pub fn compare_exchange(
        &self,
        current: State,
        new: State,
        success: Ordering,
        failure: Ordering,
    ) -> Result<State, State> {
        match self
            .0
            .compare_exchange(current.as_u8(), new.as_u8(), success, failure)
        {
            Ok(_) => Ok(current),
            Err(v) => {
                // Safety: `AtomicState`'s API only allows storing valid states.
                // Thus, we can safely convert the raw value to a `State`.
                Err(unsafe { State::from_u8_unchecked(v) })
            }
        }
    }

    #[inline]
    pub fn store(&self, value: State, order: Ordering) {
        self.0.store(value.as_u8(), order);
    }
}

/// A structure providing a one-time initialization routine.
///
/// Contrary to a `MUMcsLock`, this structure is not a lock, thus it does
/// NOT provide interior mutability. It is used to perform a one-time
/// initialization of a value, and then provide a reference to it.
pub struct Once<T> {
    state: AtomicState,
    value: UnsafeCell<MaybeUninit<T>>,
}

// Safety:
// `Once` only provides an immutable reference to the value when initialized.
// On initialization, we manually make sure there are no data races.
unsafe impl<T: Send> Send for Once<T> {}
unsafe impl<T: Send + Sync> Sync for Once<T> {}

impl<T> Once<T> {
    #[must_use]
    #[inline]
    /// Creates a new `Once` structure in an uninitialized state.
    pub const fn uninit() -> Self {
        Self {
            state: AtomicState::uninit(),
            value: UnsafeCell::new(MaybeUninit::uninit()),
        }
    }

    #[must_use]
    #[inline]
    /// Creates a new `Once` structure in an initialized state.
    pub const fn from_init(value: T) -> Self {
        Self {
            state: AtomicState::new(State::Initialized),
            value: UnsafeCell::new(MaybeUninit::new(value)),
        }
    }

    #[must_use]
    #[inline]
    /// Returns true if the value has been initialized.
    pub fn is_initialized(&self) -> bool {
        self.state.load(Ordering::Acquire) == State::Initialized
    }

    #[must_use]
    #[inline]
    /// Returns true if the value has been poisoned.
    pub fn is_poisoned(&self) -> bool {
        self.state.load(Ordering::Acquire) == State::Poisoned
    }

    /// Initializes the value if it has not been initialized yet.
    ///
    /// Try to make the `initializer` function as less likely to panic as possible.
    ///
    /// If the value is still initializing, the function will NOT wait until initialization is complete.
    /// To do so, use `get`.
    pub fn call_once<F>(&self, initializer: F)
    where
        F: FnOnce() -> T,
    {
        if self
            .state
            .compare_exchange(
                State::Uninitialized,
                State::Initializing,
                Ordering::Acquire,
                Ordering::Relaxed,
            )
            .is_ok()
        {
            // It is our job to initialize it
            let initialized_value = PoisonGuard::guard_call(self, initializer);

            // Safety:
            // Thanks to `self.state`, we are the only one accessing the value right now.
            unsafe { (*self.value.get()).write(initialized_value) };

            self.state.store(State::Initialized, Ordering::Release);
        }
    }

    #[must_use]
    #[track_caller]
    /// Returns a reference to the value if it has been initialized.
    ///
    /// If the value is still initializing, this function will block until initialization is complete.
    ///
    /// # Panics
    ///
    /// If initialization fails, the value will be marked as poisoned and a panic will occur.
    /// This behavior depends on panic unwinding, so it does not work in `no_std` environments
    /// with `panic = "abort"`.
    pub fn get(&self) -> Option<&T> {
        #[cold]
        #[track_caller]
        fn poisoned() -> ! {
            panic!("Once is poisoned, cannot get value");
        }

        match self.state.load(Ordering::Acquire) {
            State::Initialized => {
                // Safety:
                // We have ensured that the value is initialized.
                Some(unsafe { (*self.value.get()).assume_init_ref() })
            }
            State::Initializing => {
                while self.state.load(Ordering::Acquire) == State::Initializing {
                    core::hint::spin_loop();
                }

                let state = self.state.load(Ordering::Acquire);
                if state == State::Initialized {
                    Some(unsafe { (*self.value.get()).assume_init_ref() })
                } else {
                    debug_assert_eq!(state, State::Poisoned);
                    poisoned();
                }
            }
            State::Uninitialized => None,
            State::Poisoned => poisoned(),
        }
    }
}

impl<T> Drop for Once<T> {
    fn drop(&mut self) {
        if self.state.load(Ordering::Acquire) == State::Initialized {
            // Safety:
            // We are the only one accessing the value right now (dropping)
            // AND the value is initialized (if-statement).
            unsafe { self.value.get_mut().assume_init_drop() };
        }
    }
}

impl<T> Poisonable for Once<T> {
    #[inline]
    fn poison(&self) {
        self.state.store(State::Poisoned, Ordering::Release);
    }
}

trait Poisonable {
    fn poison(&self);
}

/// A guard structure that marks the inner value as poisoned when dropped.
struct PoisonGuard<'a, P: Poisonable>(&'a P);

impl<P: Poisonable> PoisonGuard<'_, P> {
    #[inline]
    /// Call this function when initialization is successful.
    const fn init_success(self) {
        // Note: No memory is leaked here.
        core::mem::forget(self);
    }

    #[must_use]
    #[inline]
    /// Safely call the given function and return the initialized value.
    ///
    /// On success, the behavior is transparent.
    /// On failure, the given `Once` is marked as poisoned.
    pub fn guard_call<T, F: FnOnce() -> T>(poisonable: &P, f: F) -> T {
        let poison_guard = PoisonGuard(poisonable);

        let initialized_value = f();

        poison_guard.init_success();

        initialized_value
    }
}

impl<P: Poisonable> Drop for PoisonGuard<'_, P> {
    #[inline]
    fn drop(&mut self) {
        self.0.poison();
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::sync::{Arc, Barrier};
    use std::thread::spawn;

    #[test]
    fn test_once() {
        let once = Once::uninit();
        assert!(once.get().is_none());
        assert!(!once.is_initialized());

        once.call_once(|| 42);
        assert!(once.is_initialized());

        let value = once.get().unwrap();
        assert_eq!(*value, 42);
    }

    #[test]
    fn test_once_init() {
        let once = Once::from_init(42);
        assert!(once.is_initialized());
        once.call_once(|| panic!("This should not be called"));
        let value = once.get().unwrap();
        assert_eq!(*value, 42);
    }

    #[test]
    fn test_once_only_once() {
        let once = Once::uninit();

        once.call_once(|| 42);
        once.call_once(|| panic!("This should not be called"));
    }

    #[test]
    fn test_once_drop() {
        let once = Once::uninit();
        let _once_uninit: Once<Box<u8>> = Once::uninit();

        once.call_once(|| Box::new(42));
    }

    #[test]
    fn test_once_concurrent() {
        let once = Arc::new(Once::uninit());
        once.call_once(|| 42);

        let num_threads = 10;
        let barrier = Arc::new(Barrier::new(num_threads));

        let mut handles = Vec::with_capacity(num_threads);

        for _ in 0..num_threads {
            let once = once.clone();
            let barrier = barrier.clone();

            handles.push(spawn({
                let once = once.clone();
                let barrier = barrier.clone();
                move || {
                    let once_value = once.get().unwrap();
                    barrier.wait();
                    assert_eq!(*once_value, 42);
                    drop(once)
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }

    #[test]
    fn test_once_poison() {
        let once = Arc::new(Once::uninit());

        let handle_that_panics = {
            let once = once.clone();
            spawn(move || {
                once.call_once(|| {
                    panic!("Initialization failed!");
                });
            })
        };
        handle_that_panics.join().unwrap_err();

        assert!(!once.is_initialized());
        assert!(once.is_poisoned());
    }

    #[test]
    #[should_panic(expected = "Once is poisoned, cannot get value")]
    fn test_once_poison_get() {
        let once = Arc::new(Once::uninit());

        let handle_that_panics = {
            let once = once.clone();
            spawn(move || {
                once.call_once(|| {
                    panic!("Initialization failed!");
                });
            })
        };
        handle_that_panics.join().unwrap_err();

        // Poisoning the `Once` should not leave the value in an `Initializing` state.
        // This call should finish (panic) immediately (instead of infinite loop).
        let _ = once.get();
    }

    #[test]
    fn test_call_once() {
        static COUNTER: AtomicU8 = AtomicU8::new(0);

        for _ in 0..5 {
            call_once!({
                COUNTER.fetch_add(1, Ordering::Relaxed);
            });
        }

        assert_eq!(COUNTER.load(Ordering::Relaxed), 1);
    }
}
