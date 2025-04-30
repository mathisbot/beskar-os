//! A wrapper around lazily-initialized data.
//!
//! This structure is somewhat similar to `std::sync::Once`, and it does not provide interior mutability.
//! It is used to perform a one-time initialization of a value, and then provide a reference to it.
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
//! Note that, in this case, if every thread calls the function `perform_once` at the same time,
//! non-executing threads won't block on the operation.
//! If you want to ensure the operation is complete before proceeding, use `Once::get`.
use core::cell::UnsafeCell;
use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicU8, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
/// Possible states of the `Once` structure.
enum State {
    Uninitialized,
    Initializing,
    Initialized,
}

impl State {
    #[must_use]
    #[inline]
    /// Converts the state to a `u8` value.
    const fn as_u8(self) -> u8 {
        match self {
            Self::Uninitialized => 0,
            Self::Initializing => 1,
            Self::Initialized => 2,
        }
    }

    #[must_use]
    #[inline]
    /// Converts a `u8` value to a `State`.
    /// Returns `None` if the value is not a valid state.
    const fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Uninitialized),
            1 => Some(Self::Initializing),
            2 => Some(Self::Initialized),
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
    const unsafe fn from_u8_unchecked(value: u8) -> Self {
        unsafe { Self::from_u8(value).unwrap_unchecked() }
    }
}

/// Wrapper around `AtomicU8` to provide a more convenient API.
struct AtomicState(AtomicU8);

impl AtomicState {
    #[must_use]
    #[inline]
    pub const fn uninit() -> Self {
        Self(AtomicU8::new(State::Uninitialized.as_u8()))
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
#[allow(clippy::non_send_fields_in_send_ty)]
unsafe impl<T> Send for Once<T> {}
unsafe impl<T> Sync for Once<T> {}

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
            let initialized_value = initializer();

            // Safety:
            // Thanks to `self.state`, we are the only one accessing the value right now.
            unsafe { (*self.value.get()).write(initialized_value) };

            self.state.store(State::Initialized, Ordering::Release);
        }
    }

    #[must_use]
    /// Returns a reference to the value if it has been initialized.
    ///
    /// If the value is still initializing, this function will block until initialization is complete.
    pub fn get(&self) -> Option<&T> {
        match self.state.load(Ordering::Acquire) {
            State::Initialized => {
                // Safety:
                // We have ensured that the value is initialized.
                Some(unsafe { (*self.value.get()).assume_init_ref() })
            }
            State::Initializing => {
                // Here we choose to wait instead of returning `None` if the value is being initialized.
                // It has one downside: if initialization panics, the waiting thread will be stuck forever.
                while self.state.load(Ordering::Acquire) == State::Initializing {
                    core::hint::spin_loop();
                }
                debug_assert_eq!(self.state.load(Ordering::Acquire), State::Initialized);
                Some(unsafe { (*self.value.get()).assume_init_ref() })
            }
            State::Uninitialized => None,
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

#[cfg(test)]
mod test {
    use super::*;
    use std::sync::{Arc, Barrier};
    use std::thread::spawn;

    #[test]
    fn test_once() {
        let once = Once::uninit();
        assert!(once.get().is_none());

        once.call_once(|| 42);

        let value = once.get().unwrap();
        assert_eq!(*value, 42);
    }

    #[test]
    fn test_init() {
        let once = Once::from_init(42);
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
    fn test_concurrent() {
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
}
