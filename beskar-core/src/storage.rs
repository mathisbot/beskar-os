use thiserror::Error;

#[derive(Debug, Error, Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
/// An error that can occur when performing block device operations.
pub enum BlockDeviceError {
    #[error("I/O error")]
    Io,
    #[error("Out of bounds")]
    OutOfBounds,
    #[error("Unsupported operation")]
    Unsupported,
    #[error("Unaligned access")]
    UnalignedAccess,
}

/// A trait for block devices.
///
/// These are physical devices (such as hard drives and USB sticks) that can perform
/// read and/or write operations in blocks.
pub trait BlockDevice {
    #[must_use]
    /// Logical block size in bytes.
    fn block_size(&self) -> usize;

    #[must_use]
    /// Logical block count, when the device has a fixed known capacity.
    fn block_count(&self) -> Option<u64> {
        None
    }

    /// Read blocks from the device into the given buffer.
    ///
    /// The `offset` parameter specifies the block offset from the start of the device.
    ///
    /// # Errors
    ///
    /// This function returns an error if the read operation failed
    /// or if `dst.len()` isn't a multiple of `Self::block_size`.
    fn read(&mut self, dst: &mut [u8], offset: usize) -> Result<(), BlockDeviceError>;
    /// Write blocks to the device from the given buffer.
    ///
    /// # Errors
    ///
    /// This function returns an error if the write operation failed
    /// or if `src.len()` isn't a multiple of `Self::block_size`.
    fn write(&mut self, src: &[u8], offset: usize) -> Result<(), BlockDeviceError>;
}

/// A trait for kernel devices.
///
/// These are virtual devices (such as `stdin` and `stdout`) that are not backed by any physical device.
/// They behave like `BlockDevice`s that have a block size of 1 byte.
///
/// The only purpose of this trait is to provide a `dyn`-compatible interface.
pub trait KernelDevice {
    /// Read blocks from the device into the given buffer.
    ///
    /// The `offset` parameter specifies the block offset from the start of the device.
    ///
    /// # Errors
    ///
    /// This function returns an error if the read operation failed
    fn read(&mut self, dst: &mut [u8], offset: usize) -> Result<(), BlockDeviceError>;
    /// Write blocks to the device from the given buffer.
    ///
    /// # Errors
    ///
    /// This function returns an error if the write operation failed
    fn write(&mut self, src: &[u8], offset: usize) -> Result<(), BlockDeviceError>;

    fn on_open(&mut self) {}

    fn on_close(&mut self) {}
}

impl BlockDevice for dyn KernelDevice + Send + Sync {
    #[inline]
    fn block_size(&self) -> usize {
        1
    }

    #[inline]
    fn read(&mut self, dst: &mut [u8], offset: usize) -> Result<(), BlockDeviceError> {
        KernelDevice::read(self, dst, offset)
    }

    #[inline]
    fn write(&mut self, src: &[u8], offset: usize) -> Result<(), BlockDeviceError> {
        KernelDevice::write(self, src, offset)
    }
}
