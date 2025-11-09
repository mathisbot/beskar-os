//! A wrapper around structures to provide volatile access.
//!
//! Volatile access are particularly useful when dealing with MMIO.
//!
//! ## Example
//!
//! ```rust
//! # use hyperdrive::ptrs::volatile::{Volatile, ReadWrite};
//! #
//! let mut value = 0;
//! let volatile_ptr = Volatile::<ReadWrite, _>::from_mut(&mut value);
//!
//! unsafe { volatile_ptr.write(42) }
//! assert_eq!(unsafe { volatile_ptr.read() }, 42);
//! ```
//!
//! In real world applications, you will not have to specify `::<Access, _>` as the compiler will infer it :
//!
//! ```rust
//! # use hyperdrive::ptrs::volatile::{Volatile, ReadWrite};
//! # use core::ptr::NonNull;
//! #
//! struct MyStruct {
//!     ptr: Volatile<ReadWrite, u32>,
//! }
//!
//! impl MyStruct {
//!     fn new(ptr: *mut u32) -> Self {
//!         let non_null = NonNull::new(ptr).unwrap();
//!         Self {
//!             ptr: Volatile::new(non_null), // Compiler infers `<ReadWrite, u32>`
//!         }
//!     }
//! }
//!
//! let my_struct = MyStruct::new(0xdead_beef as *mut u32);
//! ```
//!
//! Please note that in the above example, the pointer is dangling.
//! This is not UB as long as it is not dereferenced/accessed.
//!
//! As access rights are checked at compile time, the following code will not compile:
//!
//! ```rust,compile_fail,E0432
//! # use hyperdrive::volatile::{Volatile, ReadOnly};
//! # use core::ptr::NonNull;
//! #
//! let value = 0;
//! let volatile_ptr = Volatile::<ReadOnly, _>::from_ref(&value);
//!
//! unsafe { volatile_ptr.write(42) }
//! ```

use core::{marker::PhantomData, ptr::NonNull};

trait Sealed {}
#[expect(private_bounds, reason = "Forbid impl `Access`")]
pub trait Access: Sealed {}

pub trait ReadAccess: Access {}
pub trait WriteAccess: Access {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoAccess;
impl Sealed for NoAccess {}
impl Access for NoAccess {}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReadOnly;
impl Sealed for ReadOnly {}
impl Access for ReadOnly {}
impl ReadAccess for ReadOnly {}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WriteOnly;
impl Sealed for WriteOnly {}
impl Access for WriteOnly {}
impl WriteAccess for WriteOnly {}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReadWrite;
impl Sealed for ReadWrite {}
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
    /// Casts the volatile wrapper to a pointer.
    pub const fn as_non_null(&self) -> NonNull<T> {
        self.ptr
    }

    #[must_use]
    #[inline]
    /// Creates a new volatile pointer from a mutable reference.
    pub const fn from_mut(ptr: &mut T) -> Self {
        Self::new(NonNull::from_mut(ptr))
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
    /// # Safety
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
    /// # Safety
    ///
    /// See `core::ptr::byte_add` for safety requirements.
    pub const unsafe fn byte_add(&self, offset: usize) -> Self {
        Self {
            ptr: unsafe { self.ptr.byte_add(offset) },
            _phantom: PhantomData,
        }
    }
}

impl<A: ReadAccess, T: ?Sized> Volatile<A, T> {
    #[must_use]
    #[inline]
    /// Reads the value.
    ///
    /// # Safety
    ///
    /// The inner pointer must be valid.
    pub unsafe fn read(&self) -> T
    where
        T: Sized,
    {
        unsafe { self.ptr.read_volatile() }
    }
}

impl<T: ?Sized> Volatile<ReadOnly, T> {
    #[must_use]
    #[inline]
    /// Creates a new volatile pointer from a reference.
    pub const fn from_ref(ptr: &T) -> Self {
        Self::new_read_only(NonNull::from_ref(ptr))
    }

    #[must_use]
    #[inline]
    /// Creates a new read-only volatile pointer.
    pub const fn new_read_only(ptr: NonNull<T>) -> Self {
        Self::new(ptr).change_access()
    }
}

impl<A: WriteAccess, T> Volatile<A, T> {
    #[inline]
    /// Writes the value.
    ///
    /// # Safety
    ///
    /// The inner pointer must be valid.
    pub unsafe fn write(&self, value: T) {
        unsafe { self.ptr.write_volatile(value) };
    }
}

impl<T: ?Sized> Volatile<WriteOnly, T> {
    #[must_use]
    #[inline]
    /// Creates a new write-only volatile pointer.
    pub const fn new_write_only(ptr: NonNull<T>) -> Self {
        Self::new(ptr).change_access()
    }
}

impl<A: ReadAccess + WriteAccess, T> Volatile<A, T> {
    #[inline]
    /// Updates the value.
    ///
    /// # Safety
    ///
    /// The inner pointer must be valid.
    pub unsafe fn update(&self, f: impl FnOnce(T) -> T) {
        let old = unsafe { self.ptr.read() };
        let new = f(old);
        unsafe { self.ptr.write_volatile(new) };
    }
}

impl<T: ?Sized> Volatile<ReadWrite, T> {
    #[must_use]
    #[inline]
    /// Creates a new read-write volatile pointer.
    pub const fn new_read_write(ptr: NonNull<T>) -> Self {
        Self::new(ptr).change_access()
    }
}

mod private {
    use super::{ReadOnly, ReadWrite, Volatile, WriteOnly};

    const _: () = assert!(
        size_of::<Volatile<ReadOnly, ()>>() == size_of::<*mut ()>()
            && size_of::<Volatile<ReadOnly, u8>>() == size_of::<*mut u8>()
            && size_of::<Volatile<ReadOnly, [u8]>>() == size_of::<*mut [u8]>()
            && size_of::<Volatile<WriteOnly, ()>>() == size_of::<Volatile<ReadOnly, ()>>()
            && size_of::<Volatile<ReadWrite, ()>>() == size_of::<Volatile<ReadOnly, ()>>()
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_volatile() {
        let mut array = [0, 1, 2];
        let ptr = NonNull::new(array.as_mut_ptr()).unwrap();
        let volatile = Volatile::new_read_write(ptr);
        assert_eq!(unsafe { volatile.read() }, 0);
        unsafe { volatile.write(42) };
        assert_eq!(unsafe { volatile.read() }, 42);
        assert_eq!(array[0], 42);

        let read_only = volatile.change_access::<ReadOnly>();
        let ro_u32 = read_only.cast::<u32>();
        assert_eq!(unsafe { ro_u32.read() }, 42);
    }

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
