#![no_std]
#![forbid(unsafe_op_in_unsafe_fn)]
#![warn(clippy::pedantic, clippy::nursery)]

extern crate alloc;
use thiserror::Error;

pub mod fs;
pub mod partition;
pub mod vfs;

#[derive(Debug, Error, Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub enum DeviceError {
    #[error("An I/O error occurred")]
    IoError,
}

pub trait BlockDevice {
    const BLOCK_SIZE: usize = 512;

    /// Read blocks from the device into the given buffer.
    ///
    /// The `offset` parameter specifies the block offset from the start of the device.
    /// The `count` parameter specifies the number of blocks to read.
    ///
    /// ## Errors
    ///
    /// This function returns an error if the read operation failed.
    fn read(&self, dst: &mut [u8], offset: usize, count: usize) -> Result<(), DeviceError>;
    /// Write blocks to the device from the given buffer.
    ///
    /// `src` must have a size multiple of `BLOCK_SIZE` (defaults to 512).
    fn write(&self, src: &[u8], offset: usize) -> Result<(), DeviceError>;
}
