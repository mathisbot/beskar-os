//! Read-Write Lock
//!
//! A simple implementation of a read-write lock.
//! It is an evolution of the spinlock, where multiple readers can access the data at the same time.
//!
//! ## Examples
//!
//! Reads only:
//!
//! ```rust
//! # use hyperdrive::locks::rw::RwLock;
//! #
//! let lock = RwLock::new(0);
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
//! #
//! let lock = RwLock::new(0);
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

use core::{
    cell::UnsafeCell,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};

/// A read-write lock.
pub struct RwLock<T> {
    /// The data protected by the lock.
    data: UnsafeCell<T>,
    /// The state of the lock.
    state: AtomicState,
}

// Safety:
// `RwLock` is a synchronization primitive.
unsafe impl Send for RwLock<()> {}
unsafe impl Sync for RwLock<()> {}

impl<T> RwLock<T> {
    #[must_use]
    pub fn new(data: T) -> Self {
        Self {
            data: UnsafeCell::new(data),
            state: AtomicState::default(),
        }
    }

    #[must_use]
    pub fn read(&self) -> RwLockReadGuard<T> {
        self.state.read_lock();
        RwLockReadGuard { lock: self }
    }

    #[must_use]
    pub fn write(&self) -> RwLockWriteGuard<T> {
        self.state.write_lock();
        RwLockWriteGuard { lock: self }
    }
}

/// A guard that allows read-only access to the data.
pub struct RwLockReadGuard<'a, T> {
    lock: &'a RwLock<T>,
}

impl<T> core::ops::Deref for RwLockReadGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        debug_assert!(!self.lock.state.writer.load(Ordering::Acquire));
        unsafe { &*self.lock.data.get() }
    }
}

impl<T> Drop for RwLockReadGuard<'_, T> {
    fn drop(&mut self) {
        self.lock.state.read_unlock();
    }
}

/// A guard that allows mutable access to the data.
pub struct RwLockWriteGuard<'a, T> {
    lock: &'a RwLock<T>,
}

impl<T> core::ops::Deref for RwLockWriteGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.lock.data.get() }
    }
}

impl<T> core::ops::DerefMut for RwLockWriteGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        debug_assert_eq!(self.lock.state.readers.load(Ordering::Acquire), 0);
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<T> Drop for RwLockWriteGuard<'_, T> {
    fn drop(&mut self) {
        self.lock.state.write_unlock();
    }
}

/// The state of the lock.
struct AtomicState {
    /// The number of readers.
    readers: AtomicUsize,
    /// Whether a writer has acquired the lock.
    writer: AtomicBool,
}

impl Default for AtomicState {
    fn default() -> Self {
        Self {
            readers: AtomicUsize::new(0),
            writer: AtomicBool::new(false),
        }
    }
}

impl AtomicState {
    pub fn read_lock(&self) {
        loop {
            while self.writer.load(Ordering::Acquire) {
                core::hint::spin_loop();
            }

            // TRY to acquire the lock
            self.readers.fetch_add(1, Ordering::Acquire);

            // We give the priority to the writer:
            // if he acquired it before us, we give it the lock
            if self.writer.load(Ordering::Acquire) {
                self.readers.fetch_sub(1, Ordering::Release);
            } else {
                break;
            }
        }
    }

    #[inline]
    pub fn read_unlock(&self) {
        debug_assert_ne!(self.readers.load(Ordering::Acquire), 0);
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
            core::hint::spin_loop();
        }

        // Wait until there are no more readers
        while self.readers.load(Ordering::Acquire) != 0 {
            core::hint::spin_loop();
        }
    }

    #[inline]
    pub fn write_unlock(&self) {
        debug_assert!(self.writer.load(Ordering::Acquire));
        self.writer.store(false, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read() {
        let lock = RwLock::new(0);

        let r1 = lock.read();
        let r2 = lock.read();

        assert_eq!(*r1, 0);
        assert_eq!(*r2, 0);
    }

    #[test]
    fn write() {
        let lock = RwLock::new(0);

        let mut w = lock.write();

        assert_eq!(*w, 0);
        *w = 1;
        assert_eq!(*w, 1);
    }

    #[test]
    fn read_write() {
        let lock = RwLock::new(0);

        {
            let _r = lock.read();
        }

        let w = lock.write();
        assert_eq!(*w, 0);
    }

    #[test]
    fn write_read() {
        let lock = RwLock::new(0);

        {
            let mut w = lock.write();
            *w = 1;
        }

        let r = lock.read();
        assert_eq!(*r, 1);
    }
}
