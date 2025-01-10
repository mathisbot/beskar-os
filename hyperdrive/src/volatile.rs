//! A wrapper around structures to provide volatile access.
//!
//! Volatile access are particularly useful when dealing with MMIO.
//!
//! ## Example
//!
//! ```rust
//! # use hyperdrive::volatile::{Volatile, ReadWrite};
//! #
//! let mut value = 0;
//! let volatile_ptr = Volatile::<ReadWrite, _>::from_mut(&mut value);
//!
//! unsafe { volatile_ptr.write(42) }
//! assert_eq!(unsafe { volatile_ptr.read() }, 42);
//! ```
//!
//! As access rights are checked at compile time, the following code will not compile:
//!
//! ```rust,compile_fail
//! # use hyperdrive::volatile::{Volatile, ReadOnly};
//! # use core::ptr::NonNull;
//! #
//! let value = 0;
//! let volatile_ptr = Volatile::<ReadOnly, _>::from_ref(&value);
//!
//! unsafe { volatile_ptr.write(42) }
//! ```

use core::{marker::PhantomData, ptr::NonNull};

pub trait Access {}

pub trait ReadAccess: Access {}
pub trait WriteAccess: Access {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReadOnly;
impl Access for ReadOnly {}
impl ReadAccess for ReadOnly {}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WriteOnly;
impl Access for WriteOnly {}
impl WriteAccess for WriteOnly {}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReadWrite;
impl Access for ReadWrite {}
impl ReadAccess for ReadWrite {}
impl WriteAccess for ReadWrite {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// A wrapper around structures to provide volatile access.
pub struct Volatile<A: Access, T: ?Sized> {
    ptr: NonNull<T>,
    _phantom: PhantomData<A>,
}

impl<A: Access, T: ?Sized> Volatile<A, T> {
    #[must_use]
    #[inline]
    /// Creates a new volatile pointer.
    pub const fn new(ptr: NonNull<T>) -> Self {
        Self {
            ptr,
            _phantom: PhantomData,
        }
    }

    #[must_use]
    #[inline]
    /// Creates a new volatile pointer from a mutable reference.
    pub const fn from_mut(ptr: &mut T) -> Self {
        Self {
            ptr: unsafe { NonNull::new_unchecked(ptr) },
            _phantom: PhantomData,
        }
    }

    #[must_use]
    #[inline]
    /// Casts the volatile wrapper to another type.
    pub const fn cast<U>(&self) -> Volatile<A, U> {
        Volatile {
            ptr: self.ptr.cast(),
            _phantom: PhantomData,
        }
    }

    #[must_use]
    #[inline]
    /// Adds an offset to the volatile wrapper.
    ///
    /// Units of `offset` are in terms of `T`. To add bytes, use `byte_add`.
    ///
    /// ## Safety
    ///
    /// See `core::ptr::add` for safety requirements.
    pub const unsafe fn add(&self, offset: usize) -> Self
    where
        T: Sized,
    {
        Self {
            ptr: unsafe { self.ptr.add(offset) },
            _phantom: PhantomData,
        }
    }

    #[must_use]
    #[inline]
    /// Adds an offset to the volatile wrapper.
    ///
    /// Units of `offset` are in terms of bytes. To add in units of `T`, use `add`.
    ///
    /// ## Safety
    ///
    /// See `core::ptr::byte_add` for safety requirements.
    pub const unsafe fn byte_add(&self, offset: usize) -> Self {
        Self {
            ptr: unsafe { self.ptr.byte_add(offset) },
            _phantom: PhantomData,
        }
    }

    #[must_use]
    #[inline]
    /// Casts the volatile wrapper to a pointer.
    pub const fn as_non_null(&self) -> NonNull<T> {
        self.ptr
    }

    #[must_use]
    #[inline]
    /// Change the access permissions of the volatile pointer.
    pub const fn change_access<B: Access>(&self) -> Volatile<B, T> {
        Volatile {
            ptr: self.ptr,
            _phantom: PhantomData,
        }
    }
}

impl<A: ReadAccess, T: ?Sized> Volatile<A, T> {
    #[must_use]
    #[inline]
    /// Creates a new volatile pointer from a reference.
    pub const fn from_ref(ptr: &T) -> Self {
        let ptr_mut = core::ptr::from_ref(ptr).cast_mut();
        Self {
            ptr: unsafe { NonNull::new_unchecked(ptr_mut) },
            _phantom: PhantomData,
        }
    }

    #[must_use]
    #[inline]
    /// Reads the value.
    ///
    /// ## Safety
    ///
    /// The inner pointer must be valid.
    pub unsafe fn read(&self) -> T
    where
        T: Sized,
    {
        unsafe { self.ptr.read_volatile() }
    }
}

impl<A: WriteAccess, T> Volatile<A, T> {
    #[inline]
    /// Writes the value.
    ///
    /// ## Safety
    ///
    /// The inner pointer must be valid.
    pub unsafe fn write(&self, value: T) {
        unsafe { self.ptr.write_volatile(value) };
    }
}

impl<A: ReadAccess + WriteAccess, T> Volatile<A, T> {
    #[inline]
    /// Updates the value.
    ///
    /// ## Safety
    ///
    /// The inner pointer must be valid.
    pub unsafe fn update(&self, f: impl FnOnce(T) -> T) {
        let old = unsafe { self.ptr.read() };
        let new = f(old);
        unsafe { self.ptr.write_volatile(new) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_volatile_accesses() {
        let mut value = 0;
        let volatile = Volatile::<ReadWrite, _>::from_mut(&mut value);

        assert_eq!(unsafe { volatile.read() }, 0);

        unsafe { volatile.write(1) };
        assert_eq!(unsafe { volatile.read() }, 1);

        unsafe { volatile.update(|v| v + 1) };
        assert_eq!(unsafe { volatile.read() }, 2);
    }

    #[test]
    fn test_add() {
        let array: [usize; 2] = [0, 1];
        let first = Volatile::<ReadOnly, _>::from_ref(&array[0]);
        let second = Volatile::<ReadOnly, _>::from_ref(&array[1]);

        assert_eq!(unsafe { first.add(1) }, second);
    }

    #[test]
    fn test_byte_add() {
        let array: [usize; 2] = [0, 1];
        let first = Volatile::<ReadOnly, _>::from_ref(&array[0]);
        let second = Volatile::<ReadOnly, _>::from_ref(&array[1]);

        assert_eq!(unsafe { first.byte_add(size_of::<usize>()) }, second);
    }
}
