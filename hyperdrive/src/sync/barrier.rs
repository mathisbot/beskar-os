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
    count: usize,
    current: AtomicUsize,
    passed: AtomicUsize,
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
        Self {
            count,
            current: AtomicUsize::new(0),
            passed: AtomicUsize::new(0),
        }
    }

    /// Waits for all threads to reach the barrier.
    ///
    /// This will block the current thread until all threads have reached the barrier.
    pub fn wait(&self) {
        // Maybe we are entering the barrier between opening and resetting.
        // We must wait for the barrier to be reset before we can continue.
        while self.current.load(Ordering::Acquire) >= self.count {
            core::hint::spin_loop();
        }

        let original = self.current.fetch_add(1, Ordering::Acquire);
        while self.current.load(Ordering::Acquire) < self.count {
            core::hint::spin_loop();
        }

        // In theory, we could pass through the barrier a second time while th
        // barrier is being reset. We must wait for the passed counter to be reset
        // before we can continue.
        while self.passed.load(Ordering::Acquire) >= self.count {
            core::hint::spin_loop();
        }

        self.passed.fetch_add(1, Ordering::Release);

        // Only one thread will be responsible for resetting the barrier.
        // We simply have to wait for every thread to exit the while loop,
        // and then set the counter to 0.
        if original == 0 {
            while self.passed.load(Ordering::Acquire) < self.count {
                core::hint::spin_loop();
            }
            self.current.store(0, Ordering::Release);
            self.passed.store(0, Ordering::Release);
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
