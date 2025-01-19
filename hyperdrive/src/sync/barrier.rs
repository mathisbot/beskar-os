//! A barrier that allows a fixed number of threads to synchronize.
use core::sync::atomic::{AtomicUsize, Ordering};

/// A barrier that allows a fixed number of threads to synchronize.
///
/// ## Example
///
/// `Barrier` only has two useful methods: `new` and `wait`.
///
/// ```rust
/// # use hyperdrive::sync::barrier::Barrier;
/// #
/// let barrier = Barrier::new(1);
/// barrier.wait();
/// ```
///
/// If you want to synchronize multiple threads, simply initialize
/// the barrier with the number of threads you want to synchronize.
///
/// ```rust
/// # use hyperdrive::sync::barrier::Barrier;
/// # use std::sync::Arc;
/// # use std::thread::spawn;
/// #
/// let num_threads = 10;
/// let barrier = Arc::new(Barrier::new(num_threads));
///
/// let handles = (0..num_threads)
///     .map(|_| spawn({
///         let barrier = barrier.clone();
///         move || {
///             barrier.wait();
///         }
///     }))
///     .collect::<Vec<_>>();
///
/// for handle in handles {
///     handle.join().unwrap();
/// }
/// ```
pub struct Barrier {
    /// Amount of threads that need to reach the barrier.
    count: usize,
    /// Rank counter.
    ///
    /// Used by threads to get their rank in the barrier.
    rank: AtomicUsize,
    /// Current amount of threads that are waiting.
    current: AtomicUsize,
    /// Amount of threads that have exited the barrier.
    out: AtomicUsize,
}

// Safety: Barrier is a synchronization primitive.
#[allow(clippy::non_send_fields_in_send_ty)]
unsafe impl Send for Barrier {}
unsafe impl Sync for Barrier {}

impl Barrier {
    #[must_use]
    #[inline]
    /// Creates a new barrier that allows `count` threads to synchronize.
    pub const fn new(count: usize) -> Self {
        assert!(count > 0, "Barrier must have a count greater than 0.");
        Self {
            count,
            rank: AtomicUsize::new(0),
            current: AtomicUsize::new(0),
            out: AtomicUsize::new(0),
        }
    }

    /// Waits for all threads to reach the barrier.
    ///
    /// This will block the current thread until all threads have reached the barrier.
    pub fn wait(&self) {
        while self.rank.fetch_add(1, Ordering::AcqRel) >= self.count {
            // Avoid overflow (which would be catastrophic) by waiting instead of continuously adding.
            while self.rank.load(Ordering::Acquire) >= self.count {
                core::hint::spin_loop();
            }
        }
        self.current.fetch_add(1, Ordering::AcqRel);

        // Actual waiting loop.
        while self.current.load(Ordering::Acquire) < self.count {
            core::hint::spin_loop();
        }
        let out = self.out.fetch_add(1, Ordering::AcqRel);

        // Only one thread will be responsible for resetting the barrier.
        // We simply have to wait for every thread to exit the while loop.
        if out == self.count - 1 {
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
    #[should_panic = "Barrier must have a count greater than 0."]
    fn test_barrier_0() {
        let _ = Barrier::new(0);
    }

    #[test]
    fn test_barrier_concurrent() {
        let num_threads = 10;

        let data = Arc::new(AtomicUsize::new(0));

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
    fn test_barrier_reuse_concurrent() {
        let num_threads = 2 * 10;

        let barrier = Arc::new(Barrier::new(2));

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
    fn test_barrier_concurrent_many_uses() {
        let num_threads = 5;

        let data = Arc::new(AtomicUsize::new(0));

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
