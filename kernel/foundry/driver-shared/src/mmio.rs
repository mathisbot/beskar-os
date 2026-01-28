use core::ptr::NonNull;

use hyperdrive::ptrs::volatile::{Access, ReadAccess, Volatile, WriteAccess};

#[derive(Debug, Clone, Copy)]
/// MMIO register wrapper.
///
/// This struct is a wrapper around a volatile memory-mapped I/O register,
/// providing safe access methods.
///
/// This wrapper does not provide synchronization; it is the responsibility of the user
/// to ensure safe concurrent access.
pub struct MmioRegister<A: Access, T: ?Sized>(Volatile<A, T>);

/// SAFETY: Memory-mapped I/O registers are safe to access from different threads
/// as long as accesses are properly synchronized.
/// Note that this wrapper does not provide synchronization itself; it is the
/// responsibility of the user to ensure safe concurrent access.
unsafe impl<A: Access, T: ?Sized + Send> Send for MmioRegister<A, T> {}
unsafe impl<A: Access, T: ?Sized + Sync> Sync for MmioRegister<A, T> {}

impl<A: Access, T: ?Sized> MmioRegister<A, T> {
    #[must_use]
    #[inline]
    /// Create a new MMIO register.
    pub const fn new(ptr: NonNull<T>) -> Self {
        Self(Volatile::new(ptr))
    }

    #[must_use]
    #[inline]
    /// Create a new MMIO register.
    pub const fn from_volatile(volatile: Volatile<A, T>) -> Self {
        Self(volatile)
    }

    #[must_use]
    #[inline]
    pub const fn as_volatile(&self) -> Volatile<A, T> {
        self.0
    }

    #[must_use]
    #[inline]
    /// Returns a new `MmioRegister` offset by the given number of bytes.
    ///
    /// # Safety
    ///
    /// The caller must uphold the safety requirements of `Volatile::byte_add`.
    pub const unsafe fn byte_add(&self, offset: usize) -> Self {
        Self(unsafe { self.0.byte_add(offset) })
    }

    #[must_use]
    #[inline]
    /// Returns a new `MmioRegister` offset by the given number of bytes.
    ///
    /// # Safety
    ///
    /// The caller must uphold the safety requirements of `Volatile::add`.
    pub const unsafe fn add(&self, offset: usize) -> Self
    where
        T: Sized,
    {
        Self(unsafe { self.0.add(offset) })
    }

    #[must_use]
    #[inline]
    pub const fn cast<U>(&self) -> MmioRegister<A, U> {
        MmioRegister(self.0.cast())
    }
}

impl<A: ReadAccess, T> MmioRegister<A, T> {
    #[must_use]
    #[inline]
    /// Read the value from the MMIO register.
    ///
    /// # Safety
    ///
    /// The caller must ensure that reading from the MMIO register is safe.
    /// In particular, `MmioRegister` is not synchronized but is `Send`/`Sync`,
    /// so the caller must ensure that concurrent reads/writes are safe.
    pub unsafe fn read(&self) -> T
    where
        T: Sized,
    {
        unsafe { self.0.read() }
    }
}

impl<A: WriteAccess, T> MmioRegister<A, T> {
    #[inline]
    /// Write a value to the MMIO register.
    ///
    /// # Safety
    ///
    /// The caller must ensure that writing to the MMIO register is safe.
    /// In particular, `MmioRegister` is not synchronized but is `Send`/`Sync`,
    /// so the caller must ensure that concurrent reads/writes are safe.
    pub unsafe fn write(&self, value: T) {
        unsafe { self.0.write(value) };
    }
}

impl<A: ReadAccess + WriteAccess, T> MmioRegister<A, T> {
    #[inline]
    /// Update the value of the MMIO register.
    ///
    /// # Safety
    ///
    /// The caller must ensure that reading from and writing to the MMIO register is safe.
    /// In particular, `MmioRegister` is not synchronized but is `Send`/`Sync`,
    /// so the caller must ensure that concurrent reads/writes are safe.
    pub unsafe fn update<F: FnOnce(T) -> T>(&self, f: F) {
        unsafe { self.0.update(f) };
    }

    #[must_use]
    #[inline]
    pub const fn lower_access<NewA: Access>(&self) -> MmioRegister<NewA, T> {
        MmioRegister(self.0.change_access())
    }
}
