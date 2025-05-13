//! A simple ring buffer implementation.
//!
//! This module provides a ring buffer implementation that can be used to store
//! elements in a circular manner. It is designed to be used in scenarios where
//! elements are produced and consumed at different rates, such as in producer-consumer
//! patterns.
//!
//! # Usage
//!
//! Note that the internal buffer actually needs one extra space to distinguish
//! between full and empty states.
//! This means that you should specify `SIZE` as one more than the maximum number
//! of elements you want to store in the buffer.
//!
//! ```rust
//! # use hyperdrive::queues::ring::Ring;
//! #
//! let mut ring = Ring::<4, usize>::new();
//!
//! ring.push(1);
//! ring.push(2);
//! ring.push(3);
//! // The ring is full as the usable size is 4 - 1 = 3.
//! assert!(ring.is_full());
//! let res = ring.try_push(4);
//! assert!(res.is_err());
//!
//! assert_eq!(ring.len(), 3);
//!
//! assert_eq!(ring.pop(), Some(1));
//! assert_eq!(ring.pop(), Some(2));
//! assert_eq!(ring.pop(), Some(3));
//! assert_eq!(ring.pop(), None); // Buffer is empty
//! ```
use core::mem::MaybeUninit;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct RingFullError<T>(T);

impl<T> core::fmt::Display for RingFullError<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("Ring buffer is full")
    }
}
impl<T> core::fmt::Debug for RingFullError<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("RingFullError").finish()
    }
}

impl<T> core::error::Error for RingFullError<T> {}

#[derive(Debug)]
/// A ring buffer
///
/// Note that the internal buffer actually needs one extra space to distinguish
/// between full and empty states.
/// This means that you should specify `SIZE` as one more than the maximum number
/// of elements you want to store in the buffer.
pub struct Ring<const SIZE: usize, T> {
    /// The buffer that holds the data.
    // TODO: When const generics are stable, statically increase the size.
    buffer: [MaybeUninit<T>; SIZE],
    /// The index of the next element to be read.
    read_index: usize,
    /// The index of the next element to be written.
    write_index: usize,
}

impl<const SIZE: usize, T> Default for Ring<SIZE, T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const SIZE: usize, T> Ring<SIZE, T> {
    #[must_use]
    #[inline]
    /// Creates a new ring buffer.
    pub const fn new() -> Self {
        assert!(SIZE > 0, "Ring buffer size must be greater than 0");
        Self {
            buffer: [const { MaybeUninit::uninit() }; SIZE],
            read_index: 0,
            write_index: 0,
        }
    }

    #[must_use]
    #[inline]
    /// Returns a reference to the inner buffer.
    ///
    /// At the time of calling this function, initialized elements are located
    /// between `read_index` and `write_index` (possibly wrapping around).
    pub const fn buffer(&self) -> &[MaybeUninit<T>; SIZE] {
        &self.buffer
    }

    #[must_use]
    #[inline]
    /// Returns the current position of the read index.
    pub const fn read_index(&self) -> usize {
        self.read_index
    }

    #[must_use]
    #[inline]
    /// Returns the current position of the write index.
    pub const fn write_index(&self) -> usize {
        self.write_index
    }

    #[must_use]
    #[inline]
    /// Returns the capacity of the ring buffer.
    pub const fn capacity(&self) -> usize {
        SIZE
    }

    #[must_use]
    #[inline]
    /// Returns the number of elements in the ring buffer.
    pub const fn len(&self) -> usize {
        let w_idx = self.write_index();
        let r_idx = self.read_index();
        if w_idx >= r_idx {
            w_idx - r_idx
        } else {
            self.capacity() - r_idx + w_idx
        }
    }

    #[must_use]
    #[inline]
    /// Returns true if the ring buffer is empty.
    pub const fn is_empty(&self) -> bool {
        self.read_index() == self.write_index()
    }

    #[must_use]
    #[inline]
    /// Returns true if the ring buffer is full.
    pub const fn is_full(&self) -> bool {
        self.next_write_index() == self.read_index()
    }

    #[must_use]
    #[inline]
    /// Returns the next write index.
    const fn next_write_index(&self) -> usize {
        (self.write_index + 1) % SIZE
    }

    #[must_use]
    #[inline]
    /// Returns the next read index.
    const fn next_read_index(&self) -> usize {
        (self.read_index + 1) % SIZE
    }

    /// Pushes a new value into the ring buffer.
    pub const fn push(&mut self, value: T) {
        let next_write_index = self.next_write_index();

        assert!(
            next_write_index != self.read_index(),
            "Buffer is full, cannot push new value"
        );

        self.buffer[self.write_index].write(value);
        self.write_index = next_write_index;
    }

    /// Tries to push a new value into the ring buffer.
    ///
    /// # Errors
    ///
    /// If the buffer is full, this function returns a `RingFullError` containing the value that could not be pushed.
    pub const fn try_push(&mut self, value: T) -> Result<(), RingFullError<T>> {
        let next_write_index = self.next_write_index();

        if next_write_index == self.read_index() {
            return Err(RingFullError(value));
        }

        self.buffer[self.write_index].write(value);
        self.write_index = next_write_index;

        Ok(())
    }

    #[must_use]
    /// Pops a value from the ring buffer.
    pub const fn pop(&mut self) -> Option<T> {
        if self.is_empty() {
            return None;
        }

        let element = &self.buffer[self.read_index()];
        // Safety: The pointer is valid as it is derived from a reference
        // and the pointee is initialized (see below).
        // We could use `core::mem::replace` to avoid using `unsafe`
        // but using `read` is more efficient as it avoids a useless write.
        let value = unsafe { core::ptr::read(element) };
        self.read_index = self.next_read_index();

        // Safety: Every element between `read_index` and `write_index` (possibly wrapping around) is initialized.
        // Therefore, the element at `read_index` is initialized.
        Some(unsafe { value.assume_init() })
    }
}

impl<const SIZE: usize, T> Drop for Ring<SIZE, T> {
    fn drop(&mut self) {
        while let Some(v) = self.pop() {
            drop(v);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ring() {
        let mut ring = Ring::<4, usize>::new();

        assert_eq!(ring.len(), 0);
        assert_eq!(ring.capacity(), 4);

        ring.push(1);
        ring.push(2);
        ring.push(3);

        assert_eq!(ring.len(), 3);

        assert_eq!(ring.pop(), Some(1));
        assert_eq!(ring.pop(), Some(2));
        assert_eq!(ring.pop(), Some(3));
        assert_eq!(ring.pop(), None); // Buffer is empty
    }

    #[test]
    #[should_panic = "Buffer is full, cannot push new value"]
    fn test_ring_fill() {
        let mut ring = Ring::<3, usize>::new();

        ring.push(1);
        ring.push(2);
        assert!(ring.is_full());
        ring.push(3); // This should panic
    }

    #[test]
    fn test_ring_cycle() {
        let mut ring = Ring::<3, usize>::new();

        ring.push(1);
        ring.push(2);
        assert!(ring.is_full());

        assert_eq!(ring.pop(), Some(1));
        assert_eq!(ring.pop(), Some(2));
        assert!(ring.is_empty());

        ring.push(3);
        ring.push(4);
        assert!(ring.is_full());

        assert_eq!(ring.pop(), Some(3));
        assert_eq!(ring.pop(), Some(4));
        assert!(ring.is_empty());
    }

    #[test]
    fn test_ring_try_push() {
        let mut ring = Ring::<3, usize>::new();

        let res = ring.try_push(1);
        assert!(res.is_ok());
        assert_eq!(ring.len(), 1);

        let res = ring.try_push(2);
        assert!(res.is_ok());

        let res = ring.try_push(3);
        assert!(res == Err(RingFullError(3)));
    }

    #[test]
    #[cfg(miri)]
    /// Assert that we are not double dropping any elements.
    fn test_ring_heap() {
        let mut ring = Ring::<3, Box<usize>>::new();

        ring.push(Box::new(1));
        ring.push(Box::new(2));

        assert_eq!(ring.pop(), Some(Box::new(1)));
        assert_eq!(ring.pop(), Some(Box::new(2)));
    }

    #[test]
    #[cfg(miri)]
    fn test_ring_drop() {
        let mut ring = Ring::<3, Box<usize>>::new();

        ring.push(Box::new(1));
        ring.push(Box::new(2));

        drop(ring);
    }
}
