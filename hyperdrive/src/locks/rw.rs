//! Read-Write Lock
//!
//! A simple implementation of a read-write lock.
//! It is an evolution of the spinlock, where multiple readers can access the data at the same time.
//!
//! The structure accept a generic type `T` that is the type of the data protected by the lock.
//! The second generic type `R` is the relax strategy used by the lock.
//!
//! ## Starvation-Resistance
//!
//! This implementation prioritizes writers to prevent writer starvation. When a writer arrives:
//! 1. It acquires the writer flag first (preventing new readers)
//! 2. Then waits for existing readers to finish
//!
//! This prevents the classic scenario where continuous readers starve a writer.
//!
//! ## Examples
//!
//! Reads only:
//!
//! ```rust
//! # use hyperdrive::locks::rw::RwLock;
//! # use hyperdrive::locks::Spin;
//! #
//! let lock = RwLock::<_, Spin>::new(0);
//!
//! let r1 = lock.read();
//! let r2 = lock.read();
//!
//! assert_eq!(*r1, 0);
//! assert_eq!(*r2, 0);
//! ```
//!
//! With a write:
//!
//! ```rust
//! # use hyperdrive::locks::rw::RwLock;
//! # use hyperdrive::locks::Spin;
//! #
//! let lock = RwLock::<_, Spin>::new(0);
//!
//! {
//!     let mut w = lock.write();
//!     *w = 1;
//! }
//!
//! let r = lock.read();
//!
//! assert_eq!(*r, 1);
//! ```

use super::{RelaxStrategy, Spin};
use core::{
    cell::UnsafeCell,
    marker::PhantomData,
    sync::atomic::{AtomicBool, AtomicU32, Ordering},
};

#[derive(Default)]
/// A read-write lock.
pub struct RwLock<T: ?Sized, R: RelaxStrategy = Spin> {
    /// The state of the lock.
    state: AtomicState<R>,
    /// The data protected by the lock.
    data: UnsafeCell<T>,
}

// Safety:
// `RwLock` is a synchronization primitive.
unsafe impl<T: ?Sized + Send, R: RelaxStrategy> Send for RwLock<T, R> {}
unsafe impl<T: ?Sized + Send + Sync, R: RelaxStrategy> Sync for RwLock<T, R> {}

impl<T, R: RelaxStrategy> RwLock<T, R> {
    #[must_use]
    pub const fn new(data: T) -> Self {
        Self {
            data: UnsafeCell::new(data),
            state: AtomicState::new(),
        }
    }

    #[must_use]
    #[inline]
    pub fn into_inner(self) -> T {
        self.data.into_inner()
    }
}

impl<T: ?Sized, R: RelaxStrategy> RwLock<T, R> {
    #[must_use]
    pub fn read(&self) -> ReadGuard<'_, T, R> {
        self.state.read_lock();
        ReadGuard { lock: self }
    }

    #[must_use]
    pub fn write(&self) -> WriteGuard<'_, T, R> {
        self.state.write_lock();
        WriteGuard { lock: self }
    }
}

/// A guard that allows read-only access to the data.
pub struct ReadGuard<'a, T: ?Sized, R: RelaxStrategy> {
    lock: &'a RwLock<T, R>,
}

impl<T: ?Sized, R: RelaxStrategy> core::ops::Deref for ReadGuard<'_, T, R> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.lock.data.get() }
    }
}

impl<T: ?Sized, R: RelaxStrategy> Drop for ReadGuard<'_, T, R> {
    fn drop(&mut self) {
        self.lock.state.read_unlock();
    }
}

unsafe impl<T: ?Sized + Sync, R: RelaxStrategy> Send for ReadGuard<'_, T, R> {}
unsafe impl<T: ?Sized + Sync, R: RelaxStrategy> Sync for ReadGuard<'_, T, R> {}

/// A guard that allows mutable access to the data.
pub struct WriteGuard<'a, T: ?Sized, R: RelaxStrategy> {
    lock: &'a RwLock<T, R>,
}

impl<T: ?Sized, R: RelaxStrategy> core::ops::Deref for WriteGuard<'_, T, R> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.lock.data.get() }
    }
}

impl<T: ?Sized, R: RelaxStrategy> core::ops::DerefMut for WriteGuard<'_, T, R> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<T: ?Sized, R: RelaxStrategy> Drop for WriteGuard<'_, T, R> {
    fn drop(&mut self) {
        self.lock.state.write_unlock();
    }
}

#[derive(Debug, Default)]
/// The state of the lock.
struct AtomicState<R: RelaxStrategy = Spin> {
    /// The number of readers.
    readers: AtomicU32,
    /// Whether a writer has acquired the lock.
    writer: AtomicBool,
    /// Relax strategy.
    _relax: PhantomData<R>,
}

unsafe impl<R: RelaxStrategy> Send for AtomicState<R> {}

impl<R: RelaxStrategy> AtomicState<R> {
    #[must_use]
    #[inline]
    pub const fn new() -> Self {
        Self {
            readers: AtomicU32::new(0),
            writer: AtomicBool::new(false),
            _relax: PhantomData,
        }
    }

    pub fn read_lock(&self) {
        loop {
            while self.writer.load(Ordering::Acquire) {
                R::relax();
            }

            // TRY to acquire the lock
            self.readers.fetch_add(1, Ordering::Acquire);

            // We give the priority to the writer:
            // if he acquired it before us, we give it the lock
            if self.writer.load(Ordering::Acquire) {
                self.readers.fetch_sub(1, Ordering::Relaxed);
            } else {
                break;
            }
        }
    }

    #[inline]
    pub fn read_unlock(&self) {
        self.readers.fetch_sub(1, Ordering::Release);
    }

    pub fn write_lock(&self) {
        // Acquire the lock early to avoid starvation because of readers
        // as readers give writer priority on lock acquisition.
        while self
            .writer
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            R::relax();
        }

        // Wait until there are no more readers
        while self.readers.load(Ordering::Acquire) != 0 {
            R::relax();
        }
    }

    #[inline]
    pub fn write_unlock(&self) {
        self.writer.store(false, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Barrier};
    use std::thread::spawn;

    type TestRwLock<T> = RwLock<T, Spin>;

    #[test]
    fn read() {
        let lock = TestRwLock::new(0);

        let r1 = lock.read();
        let r2 = lock.read();

        assert_eq!(*r1, 0);
        assert_eq!(*r2, 0);
    }

    #[test]
    fn write() {
        let lock = TestRwLock::new(0);

        let mut w = lock.write();

        assert_eq!(*w, 0);
        *w = 1;
        assert_eq!(*w, 1);
    }

    #[test]
    fn read_write() {
        let lock = TestRwLock::new(0);

        {
            let _r = lock.read();
        }

        let w = lock.write();
        assert_eq!(*w, 0);
    }

    #[test]
    fn write_read() {
        let lock = TestRwLock::new(0);

        {
            let mut w = lock.write();
            *w = 1;
        }

        let r = lock.read();
        assert_eq!(*r, 1);
    }

    #[test]
    fn test_concurent_writes() {
        let lock = Arc::new(TestRwLock::new(0));

        let num_threads = 10;
        let iterations = 50;

        let mut handles = Vec::with_capacity(num_threads);

        for _ in 0..num_threads {
            let lock = lock.clone();
            let handle = spawn(move || {
                for _ in 0..iterations {
                    let mut w = lock.write();
                    *w += 1;
                }
            });

            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(*lock.read(), num_threads * iterations);
    }

    #[test]
    fn test_concurent_reads() {
        let num_readers = 10;

        let lock = Arc::new(TestRwLock::new(0));
        let barrier = Arc::new(Barrier::new(num_readers));

        let mut handles = Vec::with_capacity(num_readers);

        for _ in 0..num_readers {
            let lock = lock.clone();
            let barrier = barrier.clone();
            handles.push(spawn(move || {
                let r = lock.read();
                barrier.wait();
                drop(r);
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }

    #[test]
    fn test_concurent_readwrite() {
        let lock = Arc::new(TestRwLock::new(0));
        let barrier = Arc::new(Barrier::new(2));

        let w = spawn({
            let lock = lock.clone();
            let barrier = barrier.clone();
            move || {
                let mut w = lock.write();
                barrier.wait();
                for i in 0..=100 {
                    *w = i;
                }
            }
        });

        let r = spawn({
            let lock = lock.clone();
            let barrier = barrier.clone();
            move || {
                barrier.wait();
                let r = lock.read();
                assert_eq!(*r, 100);
            }
        });

        w.join().unwrap();
        r.join().unwrap();
    }
}
