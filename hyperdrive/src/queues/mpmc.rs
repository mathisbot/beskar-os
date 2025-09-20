//! A multiple-producer multiple-consumer queue.
//!
//! This is basically an atomic ring buffer that allows multiple producers and consumers
//! to push and pop elements concurrently.
//!
//! ## Usage
//!
//! ```rust
//! # use hyperdrive::queues::mpmc::MpmcQueue;
//! #
//! let queue = MpmcQueue::<3, usize>::new();
//!
//! // Push elements into the queue
//! queue.push(1);
//! queue.push(2);
//! queue.push(3);
//!
//! // Pop elements from the queue
//! assert_eq!(queue.pop(), Some(1));
//! assert_eq!(queue.pop(), Some(2));
//! assert_eq!(queue.pop(), Some(3));
//! assert_eq!(queue.pop(), None); // Queue is empty
//! ```
use core::{
    cell::UnsafeCell,
    mem::MaybeUninit,
    sync::atomic::{AtomicUsize, Ordering},
};

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct MpmcQueueFullError<T>(T);

impl<T> core::fmt::Display for MpmcQueueFullError<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("MPMC queue buffer is full")
    }
}
impl<T> core::fmt::Debug for MpmcQueueFullError<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("MpmcQueueFullError").finish()
    }
}

impl<T> core::error::Error for MpmcQueueFullError<T> {}

#[derive(Debug)]
/// A multiple-producer multiple-consumer queue.
pub struct MpmcQueue<const SIZE: usize, T> {
    /// The buffer that holds the data.
    buffer: [Slot<T, SIZE>; SIZE],
    /// The index of the next element to be read.
    read_index: AtomicUsize,
    /// The index of the next element to be written.
    write_index: AtomicUsize,
}

#[derive(Debug)]
struct Slot<T, const SIZE: usize> {
    /// The sequence number for this slot.
    sequence: AtomicUsize,
    /// The value stored in this slot.
    value: UnsafeCell<MaybeUninit<T>>,
}

impl<T, const SIZE: usize> Slot<T, SIZE> {
    #[must_use]
    #[inline]
    pub const fn new(sequence: usize) -> Self {
        Self {
            sequence: AtomicUsize::new(sequence),
            value: UnsafeCell::new(MaybeUninit::uninit()),
        }
    }

    #[inline]
    /// Writes a value to this slot and updates the sequence number.
    ///
    /// # Safety
    ///
    /// The caller must ensure that the value is valid and that no other thread
    /// is accessing this slot at the same time.
    pub unsafe fn write(&self, value: T, pos: usize) {
        unsafe { (*self.value.get()).write(value) };
        self.sequence.store(pos + 1, Ordering::Release);
    }

    #[must_use]
    #[inline]
    /// Reads a value from this slot and updates the sequence number.
    ///
    /// # Safety
    ///
    /// The caller must ensure that the value is valid and that no other thread
    /// is writing to this slot at the same time.
    pub unsafe fn read(&self, pos: usize) -> T {
        let value = unsafe { (&*self.value.get()).assume_init_read() };
        self.sequence.store(pos + SIZE, Ordering::Release);
        value
    }
}

// Safety: Data races are avoided by using atomic operations for the indices.
unsafe impl<const SIZE: usize, T> Send for MpmcQueue<SIZE, T> where T: Send {}
unsafe impl<const SIZE: usize, T> Sync for MpmcQueue<SIZE, T> where T: Sync {}

impl<const SIZE: usize, T> Default for MpmcQueue<SIZE, T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const SIZE: usize, T> MpmcQueue<SIZE, T> {
    #[must_use]
    #[inline]
    /// Creates a new multiple-producer multiple-consumer queue.
    ///
    /// # Panics
    ///
    /// If the buffer size is not greater than 0, this function will panic.
    pub fn new() -> Self {
        assert!(SIZE > 0, "MPMC queue buffer size must be greater than 0");
        Self {
            buffer: core::array::from_fn(Slot::new),
            read_index: AtomicUsize::new(0),
            write_index: AtomicUsize::new(0),
        }
    }

    #[inline]
    /// Pushes a new value into the queue.
    ///
    /// # Panics
    ///
    /// If the buffer is full, this function will panic.
    /// For a non-failing version, use `try_push`.
    pub fn push(&self, value: T) {
        self.try_push(value)
            .expect("Buffer is full, cannot push new value");
    }

    /// Tries to push a new value into the queue.
    ///
    /// # Errors
    ///
    /// If the buffer is full, this function returns a `MpmcQueueFullError` containing the value that could not be pushed.
    pub fn try_push(&self, value: T) -> Result<(), MpmcQueueFullError<T>> {
        let mut pos = self.write_index.load(Ordering::Relaxed);

        loop {
            let slot = &self.buffer[pos % SIZE];
            let seq = slot.sequence.load(Ordering::Acquire);

            match seq.cmp(&pos) {
                core::cmp::Ordering::Equal => {
                    match self.write_index.compare_exchange_weak(
                        pos,
                        pos + 1,
                        Ordering::Relaxed,
                        Ordering::Relaxed,
                    ) {
                        Ok(_old) => {
                            // Safety: We are the only thread writing to this slot.
                            unsafe {
                                slot.write(value, pos);
                            };
                            break Ok(()); // Successfully pushed the value
                        }
                        Err(current) => {
                            pos = current; // Retry with the current position
                        }
                    }
                }
                core::cmp::Ordering::Less => {
                    break Err(MpmcQueueFullError(value)); // Queue is full
                }
                core::cmp::Ordering::Greater => {
                    pos = self.write_index.load(Ordering::Relaxed); // Retry
                }
            }
        }
    }

    #[must_use]
    /// Pops a value from the queue.
    pub fn pop(&self) -> Option<T> {
        let mut pos = self.read_index.load(Ordering::Relaxed);

        loop {
            let slot = &self.buffer[pos % SIZE];
            let seq = slot.sequence.load(Ordering::Acquire);

            match seq.cmp(&(pos + 1)) {
                core::cmp::Ordering::Equal => {
                    match self.read_index.compare_exchange_weak(
                        pos,
                        pos + 1,
                        Ordering::Relaxed,
                        Ordering::Relaxed,
                    ) {
                        Ok(_old) => {
                            // Safety: We are the only thread accessing this slot.
                            let value = unsafe { slot.read(pos) };
                            break Some(value); // Successfully pushed the value
                        }
                        Err(current) => {
                            pos = current; // Retry with the current position
                        }
                    }
                }
                core::cmp::Ordering::Less => {
                    break None; // Queue is empty
                }
                core::cmp::Ordering::Greater => {
                    pos = self.read_index.load(Ordering::Relaxed); // Retry
                }
            }
        }
    }
}

impl<const SIZE: usize, T> Drop for MpmcQueue<SIZE, T> {
    fn drop(&mut self) {
        while let Some(v) = self.pop() {
            drop(v);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Barrier};
    use std::thread;

    #[test]
    fn test_mpmc() {
        let mpmc = MpmcQueue::<4, usize>::new();

        mpmc.push(1);
        mpmc.push(2);
        mpmc.push(3);

        assert_eq!(mpmc.pop(), Some(1));
        assert_eq!(mpmc.pop(), Some(2));
        assert_eq!(mpmc.pop(), Some(3));
        assert_eq!(mpmc.pop(), None); // Buffer is empty
    }

    #[test]
    #[should_panic = "Buffer is full, cannot push new value"]
    fn test_mpmc_fill() {
        let mpmc = MpmcQueue::<3, usize>::new();

        mpmc.push(1);
        mpmc.push(2);
        mpmc.push(3);
        mpmc.push(4); // This should panic
    }

    #[test]
    fn test_mpmc_cycle() {
        let mpmc = MpmcQueue::<3, usize>::new();

        mpmc.push(1);
        mpmc.push(2);

        assert_eq!(mpmc.pop(), Some(1));
        assert_eq!(mpmc.pop(), Some(2));

        mpmc.push(3);
        mpmc.push(4);

        assert_eq!(mpmc.pop(), Some(3));
        assert_eq!(mpmc.pop(), Some(4));

        assert!(mpmc.pop().is_none());
    }

    #[test]
    fn test_mpmc_try_push() {
        let mpmc = MpmcQueue::<3, usize>::new();

        let res = mpmc.try_push(1);
        assert!(res.is_ok());

        let res = mpmc.try_push(2);
        assert!(res.is_ok());

        let res = mpmc.try_push(3);
        assert!(res.is_ok());

        let res = mpmc.try_push(4);
        assert_eq!(res, Err(MpmcQueueFullError(4)));
    }

    #[test]
    #[cfg(miri)]
    /// Assert that we are not double dropping any elements.
    fn test_mpmc_heap() {
        let mpmc = MpmcQueue::<3, Box<usize>>::new();

        mpmc.push(Box::new(1));
        mpmc.push(Box::new(2));

        let popped1 = mpmc.pop().unwrap();
        let popped2 = mpmc.pop().unwrap();

        assert_eq!(*popped1, 1);
        assert_eq!(*popped2, 2);
    }

    #[test]
    #[cfg(miri)]
    fn test_mpmc_drop() {
        let mpmc = MpmcQueue::<3, Box<usize>>::new();

        mpmc.push(Box::new(1));
        mpmc.push(Box::new(2));

        drop(mpmc);
    }

    #[test]
    fn test_mpmc_concurrent() {
        let mpmc = Arc::new(MpmcQueue::<40, usize>::new());

        let num_producers = 4;
        let num_consumers = 4;
        let items_per_producer = 10;
        let total_items = num_producers * items_per_producer;
        let consumed_items = Arc::new(AtomicUsize::new(0));

        let start_barrier = Arc::new(Barrier::new(num_producers + num_consumers));

        let mut handles = vec![];

        // Spawn producer threads
        for producer_id in 0..num_producers {
            let mpmc_clone = Arc::clone(&mpmc);
            let barrier_clone = start_barrier.clone();
            let handle = thread::spawn(move || {
                barrier_clone.wait();
                for i in 0..items_per_producer {
                    let value = producer_id * items_per_producer + i;
                    mpmc_clone.push(value);
                }
            });
            handles.push(handle);
        }

        // Spawn consumer threads
        for _ in 0..num_consumers {
            let mpmc_clone = Arc::clone(&mpmc);
            let counter_clone = Arc::clone(&consumed_items);
            let barrier_clone = start_barrier.clone();
            let handle = thread::spawn(move || {
                barrier_clone.wait();
                loop {
                    if let Some(_value) = mpmc_clone.pop() {
                        counter_clone.fetch_add(1, Ordering::Relaxed);
                    } else {
                        thread::yield_now();
                    }

                    if counter_clone.load(Ordering::Relaxed) >= total_items {
                        break;
                    }
                }
            });
            handles.push(handle);
        }

        // Wait for all threads to complete
        for handle in handles {
            handle.join().unwrap();
        }

        // Verify all items were consumed
        assert_eq!(consumed_items.load(Ordering::Relaxed), total_items);
    }
}
