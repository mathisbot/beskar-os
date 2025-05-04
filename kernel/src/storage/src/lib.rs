#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_op_in_unsafe_fn)]
#![warn(clippy::pedantic, clippy::nursery)]

extern crate alloc;
use thiserror::Error;

pub mod fs;
pub mod partition;
pub mod vfs;

#[derive(Debug, Error, Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub enum DeviceError {
    #[error("I/O error")]
    Io,
    #[error("Out of bounds")]
    OutOfBounds,
    #[error("Unsupported operation")]
    Unsupported,
}

pub trait BlockDevice {
    const BLOCK_SIZE: usize;

    /// Read blocks from the device into the given buffer.
    ///
    /// The `offset` parameter specifies the block offset from the start of the device.
    ///
    /// ## Errors
    ///
    /// This function returns an error if the read operation failed
    /// or if `dst.len()` isn't a multiple or `Self::BLOCK_SIZE`.
    fn read(&mut self, dst: &mut [u8], offset: usize) -> Result<(), DeviceError>;
    /// Write blocks to the device from the given buffer.
    ///
    /// ## Errors
    ///
    /// This function returns an error if the write operation failed
    /// or if `src.len()` isn't a multiple or `Self::BLOCK_SIZE`.
    fn write(&mut self, src: &[u8], offset: usize) -> Result<(), DeviceError>;
}

/// A trait for kernel devices.
///
/// These are virtual devices (such as `stdin` and `stdout`) that are not backed by any physical device.
/// They behave like `BlockDevice`s that have a `BLOCK_SIZE` of 1 byte.
///
/// The only purpose of this trait is to provide a `dyn`-compatible interface.
pub trait KernelDevice {
    /// Read blocks from the device into the given buffer.
    ///
    /// The `offset` parameter specifies the block offset from the start of the device.
    ///
    /// ## Errors
    ///
    /// This function returns an error if the read operation failed
    fn read(&mut self, dst: &mut [u8], offset: usize) -> Result<(), DeviceError>;
    /// Write blocks to the device from the given buffer.
    ///
    /// ## Errors
    ///
    /// This function returns an error if the write operation failed
    fn write(&mut self, src: &[u8], offset: usize) -> Result<(), DeviceError>;
}

impl<T: KernelDevice> BlockDevice for T {
    const BLOCK_SIZE: usize = 1;

    fn read(&mut self, dst: &mut [u8], offset: usize) -> Result<(), DeviceError> {
        self.read(dst, offset)
    }

    fn write(&mut self, src: &[u8], offset: usize) -> Result<(), DeviceError> {
        self.write(src, offset)
    }
}
