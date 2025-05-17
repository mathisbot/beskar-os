//! A barrier that allows a fixed number of threads to synchronize.
//!
//! ## Example
//!
//! `Barrier` only has two useful methods: `new` and `wait`.
//!
//! ```rust
//! # use hyperdrive::sync::barrier::Barrier;
//! #
//! let barrier = Barrier::new(1);
//! barrier.wait();
//! ```
//!
//! If you want to synchronize multiple threads, simply initialize
//! the barrier with the number of threads you want to synchronize.
//!
//! ```rust
//! # use hyperdrive::sync::barrier::Barrier;
//! # use std::sync::Arc;
//! # use std::thread::spawn;
//! #
//! let num_threads = 10;
//! let barrier = Arc::new(Barrier::new(num_threads));
//!
//! let handles = (0..num_threads)
//!     .map(|_| spawn({
//!         let barrier = barrier.clone();
//!         move || {
//!             barrier.wait();
//!         }
//!     }))
//!     .collect::<Vec<_>>();
//!
//! for handle in handles {
//!     handle.join().unwrap();
//! }
//! ```
//!
//! If you need a reusable barrier, consider using `ReusableBarrier`.
//!
//! ```rust
//! # use hyperdrive::sync::barrier::ReusableBarrier;
//! # use std::sync::Arc;
//! # use std::thread::spawn;
//! #
//! let num_threads = 10;
//! let barrier = Arc::new(ReusableBarrier::new(num_threads / 2));
//!
//! let handles = (0..num_threads)
//!     .map(|_| spawn({
//!         let barrier = barrier.clone();
//!         move || {
//!             barrier.wait();
//!         }
//!     }))
//!     .collect::<Vec<_>>();
//!
//! for handle in handles {
//!     handle.join().unwrap();
//! }
//! ```
use core::sync::atomic::{AtomicU16, Ordering};

/// A barrier that allows a fixed number of threads to synchronize.
///
/// Due to its simplicity, this barrier is not reusable.
/// Attempting to call `wait` with a different number of threads than the one
/// specified in the constructor will result in a panic.
///
/// If you need reusability, use `ReusableBarrier` instead.
pub struct Barrier {
    /// Amount of threads that need to reach the barrier.
    count: AtomicU16,
    /// Current amount of threads that are waiting.
    current: AtomicU16,
}

impl Barrier {
    #[must_use]
    #[inline]
    /// Creates a new barrier that allows `count` threads to synchronize.
    ///
    /// # Panics
    ///
    /// Panics if `count` is 0.
    pub const fn new(count: u16) -> Self {
        assert!(count > 0, "Barrier must have a count greater than 0.");
        Self {
            count: AtomicU16::new(count),
            current: AtomicU16::new(0),
        }
    }

    /// Waits for all threads to reach the barrier.
    ///
    /// This will block the current thread until all threads have reached the barrier.
    ///
    /// # Panics
    ///
    /// Panics if the barrier has already been released.
    /// This means that the number of threads that have reached the barrier is greater than the
    /// number of threads that were specified in the constructor.
    pub fn wait(&self) {
        let count = self.count.load(Ordering::Relaxed);

        let curr = self.current.fetch_add(1, Ordering::Acquire);
        assert!(curr < count, "Barrier has already been released.");

        // Actual waiting loop.
        while self.current.load(Ordering::Acquire) < count {
            core::hint::spin_loop();
        }
    }
}

/// A barrier that allows a fixed number of threads to synchronize.
///
/// Thanks to its increased size (and complexity), this barrier is reusable.
/// This means that once the barrier is released, it can instantly be reused
/// without having to wait for all threads to exit the barrier.
///
/// If you do not need reusability, consider using `Barrier` (half the size) instead.
pub struct ReusableBarrier {
    /// Amount of threads that need to reach the barrier.
    count: AtomicU16,
    /// Rank counter.
    ///
    /// Used by threads to get their rank in the barrier.
    rank: AtomicU16,
    /// Current amount of threads that are waiting.
    current: AtomicU16,
    /// Amount of threads that have exited the barrier.
    out: AtomicU16,
}

impl ReusableBarrier {
    #[must_use]
    #[inline]
    /// Creates a new barrier that allows `count` threads to synchronize.
    ///
    /// # Panics
    ///
    /// Panics if `count` is 0.
    pub const fn new(count: u16) -> Self {
        assert!(count > 0, "Barrier must have a count greater than 0.");
        Self {
            count: AtomicU16::new(count),
            rank: AtomicU16::new(0),
            current: AtomicU16::new(0),
            out: AtomicU16::new(0),
        }
    }

    /// Waits for all threads to reach the barrier.
    ///
    /// This will block the current thread until all threads have reached the barrier.
    pub fn wait(&self) {
        let count = self.count.load(Ordering::Relaxed);

        while self.rank.fetch_add(1, Ordering::Acquire) >= count {
            // Avoid overflow (which would be catastrophic) by waiting instead of continuously adding.
            while self.rank.load(Ordering::Acquire) >= count {
                core::hint::spin_loop();
            }
        }
        self.current.fetch_add(1, Ordering::AcqRel);

        // Actual waiting loop.
        while self.current.load(Ordering::Acquire) < count {
            core::hint::spin_loop();
        }
        let out = self.out.fetch_add(1, Ordering::AcqRel);

        // Only one thread will be responsible for resetting the barrier.
        // We simply have to wait for every thread to exit the while loop.
        if out == count - 1 {
            self.current.store(0, Ordering::Release);
            self.out.store(0, Ordering::Release);
            // Release the barrier
            self.rank.store(0, Ordering::Release);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread::spawn;

    #[test]
    fn test_barrier() {
        let barrier = Barrier::new(1);

        barrier.wait();
    }

    #[test]
    fn test_rbarrier() {
        let barrier = ReusableBarrier::new(1);

        barrier.wait();
    }

    #[test]
    #[should_panic = "Barrier must have a count greater than 0."]
    fn test_barrier_0() {
        let _ = Barrier::new(0);
    }

    #[test]
    #[should_panic = "Barrier must have a count greater than 0."]
    fn test_rbarrier_0() {
        let _ = ReusableBarrier::new(0);
    }

    #[test]
    #[should_panic = "Barrier has already been released."]
    fn test_barrier_reuse() {
        let barrier = Barrier::new(1);

        barrier.wait();
        barrier.wait();
    }

    #[test]
    fn test_rbarrier_reuse() {
        let barrier = ReusableBarrier::new(1);

        barrier.wait();
        barrier.wait();
    }

    #[test]
    fn test_barrier_concurrent() {
        let num_threads = 10;

        let data = Arc::new(AtomicU16::new(0));

        let barrier = Arc::new(Barrier::new(num_threads));
        let handles = (0..num_threads)
            .map(|_| {
                spawn({
                    let barrier = barrier.clone();
                    let data = data.clone();
                    move || {
                        assert_eq!(data.load(Ordering::Relaxed), 0);
                        barrier.wait();
                        data.fetch_add(1, Ordering::Relaxed);
                    }
                })
            })
            .collect::<Vec<_>>();

        for handle in handles {
            handle.join().unwrap();
        }
    }

    #[test]
    fn test_rbarrier_concurrent() {
        let num_threads = 10;

        let data = Arc::new(AtomicU16::new(0));

        let barrier = Arc::new(ReusableBarrier::new(num_threads));
        let handles = (0..num_threads)
            .map(|_| {
                spawn({
                    let barrier = barrier.clone();
                    let data = data.clone();
                    move || {
                        assert_eq!(data.load(Ordering::Relaxed), 0);
                        barrier.wait();
                        data.fetch_add(1, Ordering::Relaxed);
                    }
                })
            })
            .collect::<Vec<_>>();

        for handle in handles {
            handle.join().unwrap();
        }
    }

    #[test]
    fn test_rbarrier_reuse_concurrent() {
        let num_threads = 2 * 10;

        let barrier = Arc::new(ReusableBarrier::new(2));

        let handles = (0..num_threads)
            .map(|_| {
                spawn({
                    let barrier = barrier.clone();
                    move || barrier.wait()
                })
            })
            .collect::<Vec<_>>();

        for handle in handles {
            handle.join().unwrap();
        }

        assert!(barrier.current.load(Ordering::Relaxed) == 0);
        assert!(barrier.out.load(Ordering::Relaxed) == 0);
    }

    #[test]
    fn test_rbarrier_concurrent_many_uses() {
        let num_threads = 5;

        let data = Arc::new(AtomicU16::new(0));

        let barrier = Arc::new(ReusableBarrier::new(num_threads));
        let handles = (0..num_threads)
            .map(|_| {
                spawn({
                    let barrier = barrier.clone();
                    let data = data.clone();
                    move || {
                        assert_eq!(data.load(Ordering::Relaxed), 0);
                        barrier.wait();
                        data.fetch_add(1, Ordering::Relaxed);
                    }
                })
            })
            .collect::<Vec<_>>();

        for handle in handles {
            handle.join().unwrap();
        }

        let handles = (0..num_threads)
            .map(|_| {
                spawn({
                    let barrier = barrier.clone();
                    let data = data.clone();
                    move || {
                        assert_eq!(data.load(Ordering::Relaxed), num_threads);
                        barrier.wait();
                        data.fetch_add(1, Ordering::Relaxed);
                    }
                })
            })
            .collect::<Vec<_>>();

        for handle in handles {
            handle.join().unwrap();
        }
    }
}
