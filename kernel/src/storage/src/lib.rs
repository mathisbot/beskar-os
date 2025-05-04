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
    /// The `count` parameter specifies the number of blocks to read.
    ///
    /// ## Errors
    ///
    /// This function returns an error if the read operation failed.
    fn read(&mut self, dst: &mut [u8], offset: usize, count: usize) -> Result<(), DeviceError>;
    /// Write blocks to the device from the given buffer.
    ///
    /// `src` must have a size multiple of `Self::BLOCK_SIZE`.
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
    /// The `count` parameter specifies the number of blocks to read.
    ///
    /// ## Errors
    ///
    /// This function returns an error if the read operation failed.
    fn read(&mut self, dst: &mut [u8], offset: usize) -> Result<(), DeviceError>;
    /// Write blocks to the device from the given buffer.
    ///
    /// `src` must have a size multiple of `Self::BLOCK_SIZE`.
    fn write(&mut self, src: &[u8], offset: usize) -> Result<(), DeviceError>;
}
