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
#[allow(clippy::non_send_fields_in_send_ty)]
unsafe impl<T> Send for RwLock<T> {}
unsafe impl<T> Sync for RwLock<T> {}

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
                self.readers.fetch_sub(1, Ordering::Acquire);
            } else {
                break;
            }
        }
    }

    #[inline]
    pub fn read_unlock(&self) {
        debug_assert_ne!(self.readers.load(Ordering::Relaxed), 0);
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
        debug_assert!(self.writer.load(Ordering::Relaxed));
        self.writer.store(false, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Barrier};
    use std::thread::spawn;

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

    #[test]
    fn test_concurent_writes() {
        let lock = Arc::new(RwLock::new(0));

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

        let lock = Arc::new(RwLock::new(0));
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
        let lock = Arc::new(RwLock::new(0));
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

    #[test]
    fn test_write_starvation() {
        let lock = Arc::new(RwLock::new(0));

        let barrier = Arc::new(Barrier::new(2));

        // Bih thread that will continuously start readers that will try to
        // starve the writer
        let big_handle = spawn({
            let lock = lock.clone();
            let barrier = barrier.clone();

            let reader_closure = |lock: Arc<RwLock<i32>>, tx: std::sync::mpsc::Sender<i32>| {
                let r = lock.read();
                tx.send(*r).unwrap();
                drop(r);
            };

            move || {
                let (tx, rx) = std::sync::mpsc::channel();

                let mut handle_first = spawn({
                    let lock = lock.clone();
                    let tx = tx.clone();
                    move || reader_closure(lock, tx)
                });
                barrier.wait(); // Tell main thread we are ready
                #[allow(unused_assignments)] // It IS used
                let mut handle_second = None;

                // Stop when writer has successfully written
                while let Ok(0) = rx.recv() {
                    handle_second = Some(spawn({
                        let lock = lock.clone();
                        let tx = tx.clone();
                        move || reader_closure(lock, tx)
                    }));
                    handle_first.join().unwrap();
                    handle_first = handle_second.take().unwrap();
                }

                handle_first.join().unwrap();
            }
        });

        barrier.wait(); // Wait for the reader to start

        let w = spawn({
            let lock = lock.clone();
            move || {
                let mut w = lock.write();
                *w = 42;
            }
        });

        big_handle.join().unwrap();
        w.join().unwrap();
    }
}
