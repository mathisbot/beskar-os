//! Ticket lock implementation.
//!
//! This module implements a ticket lock, which is a synchronization primitive
//! that provides mutual exclusion and fairness.
//!
//! It is not suitable for high contention scenarios, but it is a good
//! alternative to spin locks in low contention scenarios.
//!
//! Note that rustc currently requires that you at least specify either the back-off strategy
//! (and will infer the type of `T`) or the type of `T` (and will use the default `Spin`
//! back-off strategy).
//!
//! ```rust
//! # use hyperdrive::locks::ticket::TicketLock;
//! # use hyperdrive::locks::Spin;
//! #
//! let lock = TicketLock::<u32>::new(0); // `Spin` is used
//! let lock = TicketLock::<_, Spin>::new(0); // `T` is inferred
//! ```
//!
//! ```rust,compile_fail
//! # use hyperdrive::locks::rw::RwLock;
//! let lock = TicketLock::new(0);
//! ```
//!
//! # Example
//!
//! ```rust
//! # use hyperdrive::locks::ticket::TicketLock;
//! # use hyperdrive::locks::Spin;
//! let lock = TicketLock::<u8>::new(0);
//!
//! let mut guard = lock.lock();
//! *guard = 42;
//! assert_eq!(*guard, 42);
//! ```

use super::BackOff;
use core::{
    cell::UnsafeCell,
    marker::PhantomData,
    ops::{Deref, DerefMut},
    sync::atomic::{AtomicU32, Ordering},
};

/// A ticket lock.
///
/// This is an improved version of the spin lock that guarantees
/// fairness and avoids starvation.
///
/// However, it is not suitable for high contention scenarios.
/// In such cases, it is recommended to use MCS locks.
pub struct TicketLock<T, B: BackOff = super::Spin> {
    /// The ticket number of the next thread to acquire the lock.
    next_ticket: AtomicU32,
    /// The ticket number of the current thread holding the lock.
    now_serving: AtomicU32,
    /// The inner data protected by the lock.
    data: UnsafeCell<T>,
    /// The back-off strategy to use when the lock is contended.
    _back_off: PhantomData<B>,
}

// Safety:
// Mellor-Crummey and Scott lock is a synchronization primitive.
#[expect(
    clippy::non_send_fields_in_send_ty,
    reason = "Synchronization primitive"
)]
unsafe impl<T, B: BackOff> Send for TicketLock<T, B> {}
unsafe impl<T, B: BackOff> Sync for TicketLock<T, B> {}

impl<T, B: BackOff> TicketLock<T, B> {
    #[must_use]
    #[inline]
    /// Creates a new ticket lock.
    pub const fn new(data: T) -> Self {
        Self {
            next_ticket: AtomicU32::new(0),
            now_serving: AtomicU32::new(0),
            data: UnsafeCell::new(data),
            _back_off: PhantomData,
        }
    }

    #[must_use]
    #[inline]
    /// Locks the ticket lock and returns a guard.
    pub fn lock(&self) -> TicketGuard<'_, T, B> {
        // Get the ticket number for this thread.
        let ticket = self.next_ticket.fetch_add(1, Ordering::Acquire);

        // Wait until it's this thread's turn to acquire the lock.
        while self.now_serving.load(Ordering::Acquire) != ticket {
            B::back_off();
        }

        TicketGuard { lock: self }
    }

    #[must_use]
    #[inline]
    #[expect(clippy::mut_from_ref, reason = "Force lock")]
    /// Forces the lock to be unlocked.
    ///
    /// # Safety
    ///
    /// The caller must ensure there are no data races.
    pub unsafe fn force_lock(&self) -> &mut T {
        unsafe { &mut *self.data.get() }
    }

    #[inline]
    /// Unlocks the ticket lock.
    ///
    /// # Safety
    ///
    /// The caller must be the owner of the lock.
    unsafe fn unlock(&self) {
        self.now_serving.fetch_add(1, Ordering::Release);
    }

    #[must_use]
    #[inline]
    /// Consumes the lock and returns the inner data.
    pub fn into_inner(self) -> T {
        self.data.into_inner()
    }
}

/// RAII guard for the ticket lock.
pub struct TicketGuard<'l, T, B: BackOff> {
    lock: &'l TicketLock<T, B>,
}

impl<T, B: BackOff> Drop for TicketGuard<'_, T, B> {
    #[inline]
    fn drop(&mut self) {
        // Safety: If the guard exists, we have the lock.
        unsafe { self.lock.unlock() };
    }
}

impl<T, B: BackOff> Deref for TicketGuard<'_, T, B> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.lock.data.get() }
    }
}

impl<T, B: BackOff> DerefMut for TicketGuard<'_, T, B> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.lock.data.get() }
    }
}

#[cfg(test)]
mod tests {
    use super::super::Spin;
    use super::*;
    use std::sync::{Arc, Barrier};
    use std::thread::spawn;

    type TestTicketLock<T> = TicketLock<T, Spin>;

    #[test]
    fn test_ticket_lock() {
        let lock = TestTicketLock::new(0);

        let mut guard = lock.lock();
        *guard = 42;
        assert_eq!(*guard, 42);
    }

    #[test]
    fn test_ticket_lock_concurrent() {
        let nb_threads = 10;
        let barrier = Arc::new(Barrier::new(nb_threads));
        let lock = Arc::new(TestTicketLock::new(0));

        let mut handles = Vec::new();

        for _ in 0..nb_threads {
            let lock = lock.clone();
            let barrier = barrier.clone();
            handles.push(spawn(move || {
                barrier.wait();
                let mut guard = lock.lock();
                *guard += 1;
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let guard = lock.lock();
        assert_eq!(*guard, nb_threads);
    }
}
