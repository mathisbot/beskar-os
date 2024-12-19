use core::cell::UnsafeCell;
use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicU8, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Possible states of the `Once` structure.
enum State {
    Uninitialized,
    Initializing,
    Initialized,
}

impl TryFrom<u8> for State {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Uninitialized),
            1 => Ok(Self::Initializing),
            2 => Ok(Self::Initialized),
            _ => Err(()),
        }
    }
}

impl From<State> for u8 {
    fn from(state: State) -> Self {
        match state {
            State::Uninitialized => 0,
            State::Initializing => 1,
            State::Initialized => 2,
        }
    }
}

/// Wrapper around `AtomicU8` to provide a more convenient API.
struct AtomicState(AtomicU8);

impl AtomicState {
    const fn uninit() -> Self {
        Self(AtomicU8::new(State::Uninitialized as u8))
    }

    fn load(&self, order: Ordering) -> State {
        self.0.load(order).try_into().unwrap()
    }

    fn compare_exchange(
        &self,
        current: State,
        new: State,
        success: Ordering,
        failure: Ordering,
    ) -> Result<u8, u8> {
        self.0
            .compare_exchange(current.into(), new.into(), success, failure)
    }

    fn store(&self, value: State, order: Ordering) {
        self.0.store(value.into(), order);
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

unsafe impl<T> Sync for Once<T> where T: Sync {}

impl<T> Once<T> {
    #[must_use]
    pub const fn uninit() -> Self {
        Self {
            state: AtomicState::uninit(),
            value: UnsafeCell::new(MaybeUninit::uninit()),
        }
    }

    // FIXME: What to do if the initializer panics?
    /// Initializes the value if it has not been initialized yet.
    ///
    /// Try to make the `initializer` function as less likely to panic as possible.
    pub fn call_once<F>(&self, initializer: F) -> &T
    where
        F: FnOnce() -> T,
    {
        if self.state.load(Ordering::Acquire) != State::Initialized {
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
            } else {
                // Wait for another thread to initialize the value
                while self.state.load(Ordering::Acquire) == State::Initializing {
                    core::hint::spin_loop();
                }
            }
        }

        // Safety:
        // We have ensured that the value is initialized.
        unsafe { (*self.value.get()).assume_init_ref() }
    }

    #[must_use]
    pub fn get(&self) -> Option<&T> {
        match self.state.load(Ordering::Acquire) {
            State::Initialized => {
                // Safety:
                // We have ensured that the value is initialized.
                Some(unsafe { (*self.value.get()).assume_init_ref() })
            }
            State::Initializing => {
                // Here we choose to wait instead of returning `None` if the value is being initialized.
                // It is a risky design: if initialization panics, the waiting thread will be stuck forever.
                // However, it reduces "false" panics when the value returned is unwrapped.
                // FIXME: Add a timeout mechanism to avoid being stuck forever?
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
            unsafe { (*self.value.get()).assume_init_drop() };
        }
    }
}
