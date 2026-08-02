use crate::{
    sys,
    time::{Duration, Instant},
};
use beskar_core::process::sync::FutexWaitResult;
use core::{
    cell::UnsafeCell,
    marker::PhantomData,
    ops::{Deref, DerefMut},
    sync::atomic::{AtomicU64, Ordering},
};

struct Futex;

impl Futex {
    #[must_use]
    #[inline]
    /// Wait on an address while its value equals `expected`.
    pub fn wait_on_address(value: &AtomicU64, expected: u64) -> FutexWaitResult {
        sys::sc_futex_wait(value.as_ptr(), expected, 0)
    }

    #[must_use]
    #[inline]
    /// Wait on an address while its value equals `expected`, up to `timeout`.
    pub fn wait_on_address_for(
        value: &AtomicU64,
        expected: u64,
        timeout: Duration,
    ) -> FutexWaitResult {
        if timeout == Duration::ZERO {
            return if value.load(Ordering::Acquire) == expected {
                FutexWaitResult::TimedOut
            } else {
                FutexWaitResult::ValueMismatch
            };
        }

        let timeout_us = u64::try_from(timeout.as_micros()).unwrap_or(u64::MAX);
        sys::sc_futex_wait(value.as_ptr(), expected, timeout_us)
    }

    #[must_use]
    #[inline]
    /// Wait on an address while its value equals `expected`, up to `deadline`.
    pub fn wait_on_address_until(
        value: &AtomicU64,
        expected: u64,
        deadline: Instant,
    ) -> FutexWaitResult {
        let now = crate::time::Instant::now();
        if deadline <= now {
            return if value.load(Ordering::Acquire) == expected {
                FutexWaitResult::TimedOut
            } else {
                FutexWaitResult::ValueMismatch
            };
        }

        Self::wait_on_address_for(value, expected, deadline - now)
    }

    #[inline]
    /// Wake one waiter on this futex address.
    pub fn wake_by_address_single(value: &AtomicU64) -> bool {
        let n = Self::wake_by_address_n(value, 1);
        n >= 1
    }

    #[inline]
    /// Wake all waiters on this futex address.
    pub fn wake_by_address_all(value: &AtomicU64) -> usize {
        Self::wake_by_address_n(value, usize::MAX)
    }

    #[inline]
    /// Wake up to `count` waiters on this futex address.
    pub fn wake_by_address_n(value: &AtomicU64, count: usize) -> usize {
        if count == 0 {
            return 0;
        }

        sys::sc_futex_wake(value.as_ptr(), count)
    }
}

pub struct Mutex<T: ?Sized> {
    state: AtomicU64,
    value: UnsafeCell<T>,
}

// Safety: mutex state serializes access to inner value.
unsafe impl<T: ?Sized + Send> Send for Mutex<T> {}
// Safety: lock/unlock operations ensure mutable aliasing rules are respected.
unsafe impl<T: ?Sized + Send> Sync for Mutex<T> {}

impl<T> Mutex<T> {
    #[must_use]
    #[inline]
    pub const fn new(value: T) -> Self {
        Self {
            state: AtomicU64::new(0),
            value: UnsafeCell::new(value),
        }
    }

    #[must_use]
    #[inline]
    pub fn into_inner(self) -> T {
        self.value.into_inner()
    }
}

impl<T: ?Sized> Mutex<T> {
    #[must_use]
    #[inline]
    pub fn lock(&self) -> MutexGuard<'_, T> {
        if self
            .state
            .compare_exchange(0, 1, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            self.lock_contended();
        }

        MutexGuard {
            mutex: self,
            _not_send: PhantomData,
        }
    }

    #[must_use]
    #[inline]
    pub fn try_lock(&self) -> Option<MutexGuard<'_, T>> {
        self.state
            .compare_exchange(0, 1, Ordering::Acquire, Ordering::Relaxed)
            .ok()
            .map(|_| MutexGuard {
                mutex: self,
                _not_send: PhantomData,
            })
    }

    #[must_use]
    #[inline]
    pub const fn get_mut(&mut self) -> &mut T {
        self.value.get_mut()
    }

    fn lock_contended(&self) {
        loop {
            if self.state.swap(2, Ordering::Acquire) == 0 {
                return;
            }

            let _ = Futex::wait_on_address(&self.state, 2);
        }
    }

    fn unlock(&self) {
        if self.state.swap(0, Ordering::Release) == 2 {
            let _ = Futex::wake_by_address_single(&self.state);
        }
    }
}

impl<T: Default> Default for Mutex<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

pub struct MutexGuard<'a, T: ?Sized> {
    mutex: &'a Mutex<T>,
    _not_send: PhantomData<*mut ()>,
}

impl<T: ?Sized> Deref for MutexGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        // Safety: the guard guarantees the mutex is locked.
        unsafe { &*self.mutex.value.get() }
    }
}

impl<T: ?Sized> DerefMut for MutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // Safety: mutable access is unique while the guard exists.
        unsafe { &mut *self.mutex.value.get() }
    }
}

impl<T: ?Sized> Drop for MutexGuard<'_, T> {
    fn drop(&mut self) {
        self.mutex.unlock();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WaitTimeoutResult {
    timed_out: bool,
}

impl WaitTimeoutResult {
    #[must_use]
    #[inline]
    pub const fn timed_out(self) -> bool {
        self.timed_out
    }
}

pub struct Condvar {
    sequence: AtomicU64,
}

impl Condvar {
    #[must_use]
    #[inline]
    pub const fn new() -> Self {
        Self {
            sequence: AtomicU64::new(0),
        }
    }

    #[inline]
    pub fn notify_one(&self) -> bool {
        self.sequence.fetch_add(1, Ordering::Release);
        Futex::wake_by_address_single(&self.sequence)
    }

    #[inline]
    pub fn notify_all(&self) -> usize {
        self.sequence.fetch_add(1, Ordering::Release);
        Futex::wake_by_address_all(&self.sequence)
    }

    #[inline]
    pub fn notify_n(&self, count: usize) -> usize {
        if count == 0 {
            return 0;
        }

        self.sequence.fetch_add(1, Ordering::Release);
        Futex::wake_by_address_n(&self.sequence, count)
    }

    #[must_use]
    #[inline]
    pub fn wait<'a, T>(&self, guard: MutexGuard<'a, T>) -> MutexGuard<'a, T> {
        let (guard, _) = self.wait_until_inner(guard, None);
        guard
    }

    #[must_use]
    #[inline]
    pub fn wait_for<'a, T>(
        &self,
        guard: MutexGuard<'a, T>,
        timeout: Duration,
    ) -> (MutexGuard<'a, T>, WaitTimeoutResult) {
        let deadline = crate::time::Instant::now() + timeout;
        self.wait_until_inner(guard, Some(deadline))
    }

    #[must_use]
    #[inline]
    pub fn wait_until<'a, T>(
        &self,
        guard: MutexGuard<'a, T>,
        deadline: Instant,
    ) -> (MutexGuard<'a, T>, WaitTimeoutResult) {
        self.wait_until_inner(guard, Some(deadline))
    }

    fn wait_until_inner<'a, T>(
        &self,
        guard: MutexGuard<'a, T>,
        deadline: Option<Instant>,
    ) -> (MutexGuard<'a, T>, WaitTimeoutResult) {
        let observed = self.sequence.load(Ordering::Acquire);
        let mutex = guard.mutex;
        drop(guard);

        #[expect(clippy::option_if_let_else)]
        let wake = match deadline {
            Some(deadline) => Futex::wait_on_address_until(&self.sequence, observed, deadline),
            None => Futex::wait_on_address(&self.sequence, observed),
        };

        let relocked = mutex.lock();
        let timed_out = matches!(wake, FutexWaitResult::TimedOut);

        (relocked, WaitTimeoutResult { timed_out })
    }
}

impl Default for Condvar {
    fn default() -> Self {
        Self::new()
    }
}
