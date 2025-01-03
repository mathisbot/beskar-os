//! A wrapper around structures to provide volatile access.
//!
//! Volatile access are particularly useful when dealing with MMIO.
//!
//! ## Example
//!
//! ```rust
//! # use hyperdrive::volatile::{Volatile, Access};
//! # use core::ptr::NonNull;
//! #
//! let mut value = 0;
//! let volatile_ptr = Volatile::from_mut(&mut value, Access::ReadWrite);
//!
//! unsafe { volatile_ptr.write(42) }
//! assert_eq!(unsafe { volatile_ptr.read() }, 42);
//! ```

use core::ptr::NonNull;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Access permissions for volatile memory.
pub enum Access {
    ReadOnly,
    WriteOnly,
    ReadWrite,
}

impl PartialOrd for Access {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        if self == other {
            return Some(core::cmp::Ordering::Equal);
        }

        if self == &Self::ReadWrite {
            // `other` is not `ReadWrite`, so it must be `ReadOnly` or `WriteOnly`.
            return Some(core::cmp::Ordering::Greater);
        }

        if other == &Self::ReadWrite {
            // `self` is not `ReadWrite`, so it must be `ReadOnly` or `WriteOnly`.
            return Some(core::cmp::Ordering::Less);
        }

        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// A wrapper around structures to provide volatile access.
pub struct Volatile<T>
where
    T: ?Sized,
{
    ptr: NonNull<T>,
    access: Access,
}

impl<T> Volatile<T> {
    #[must_use]
    #[inline]
    /// Creates a new volatile pointer.
    pub const fn new(ptr: NonNull<T>, access: Access) -> Self {
        Self { ptr, access }
    }

    #[must_use]
    /// Creates a new volatile pointer.
    pub const fn from_mut(ptr: &mut T, access: Access) -> Self {
        Self {
            ptr: unsafe { NonNull::new_unchecked(ptr) },
            access,
        }
    }

    #[must_use]
    /// Creates a new volatile pointer.
    pub const fn from_ref(ptr: &T) -> Self {
        let ptr = core::ptr::from_ref(ptr).cast_mut();
        Self {
            ptr: unsafe { NonNull::new_unchecked(ptr) },
            access: Access::ReadOnly,
        }
    }

    #[must_use]
    /// Reads the value.
    ///
    /// ## Panics
    ///
    /// Panics if access is `WriteOnly`
    ///
    /// ## Safety
    ///
    /// The inner pointer must be valid.
    pub unsafe fn read(&self) -> T {
        assert!(self.access >= Access::ReadOnly, "Unauthorized read access");
        unsafe { self.ptr.read_volatile() }
    }

    /// Writes the value.
    ///
    /// ## Panics
    ///
    /// Panics if access is `ReadOnly`
    ///
    /// ## Safety
    ///
    /// The inner pointer must be valid.
    pub unsafe fn write(&self, value: T) {
        assert!(
            self.access >= Access::WriteOnly,
            "Unauthorized write access"
        );
        unsafe { self.ptr.write_volatile(value) };
    }

    /// Updates the value.
    ///
    /// ## Panics
    ///
    /// Panics if access is not `ReadWrite`
    ///
    /// ## Safety
    ///
    /// The inner pointer must be valid.
    pub unsafe fn update(&self, f: impl FnOnce(T) -> T) {
        assert!(
            self.access >= Access::ReadWrite,
            "Unauthorized read-write access"
        );
        let old = unsafe { self.ptr.read() };
        let new = f(old);
        unsafe { self.ptr.write_volatile(new) };
    }

    #[must_use]
    #[inline]
    /// Casts the volatile wrapper to another type.
    pub const fn cast<U>(&self) -> Volatile<U> {
        Volatile {
            ptr: self.ptr.cast(),
            access: self.access,
        }
    }

    #[must_use]
    #[inline]
    /// Casts the volatile wrapper to a pointer.
    pub const fn as_ptr(&self) -> NonNull<T> {
        self.ptr
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_volatile_accesses() {
        let mut value = 0;
        let volatile = Volatile::from_mut(&mut value, Access::ReadWrite);

        assert_eq!(unsafe { volatile.read() }, 0);

        unsafe { volatile.write(1) };
        assert_eq!(unsafe { volatile.read() }, 1);

        unsafe { volatile.update(|v| v + 1) };
        assert_eq!(unsafe { volatile.read() }, 2);
    }

    #[test]
    #[should_panic = "Unauthorized read access"]
    fn test_unauthorized_read() {
        let mut value = 0;
        let volatile = Volatile::from_mut(&mut value, Access::WriteOnly);

        let _ = unsafe { volatile.read() };
    }

    #[test]
    #[should_panic = "Unauthorized write access"]
    fn test_unauthorized_write() {
        let value = 0;
        let volatile = Volatile::from_ref(&value);

        unsafe { volatile.write(1) };
    }
}
