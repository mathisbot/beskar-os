#![no_std]
#![forbid(unsafe_op_in_unsafe_fn)]
#![warn(clippy::pedantic, clippy::nursery)]
//! Holonet is the galactic network stack for the kernel.

extern crate alloc;
use thiserror::Error;

pub mod l2;
pub mod l3;
pub mod l4;
pub mod utils;

pub trait Nic {
    /// Get the MAC address of this network interface.
    fn mac_address(&self) -> crate::l2::ethernet::MacAddress;

    /// Poll for an incoming frame. Returns a reference to the frame data if available.
    /// The caller must call `consume_frame()` after processing the frame to release the buffer.
    /// Calling `poll_frame()` multiple times without calling `consume_frame()` will return
    /// the same frame.
    ///
    /// # Safety considerations
    ///
    /// This method takes `&self` to allow reading without exclusive access. However, the buffer
    /// must not be modified until `consume_frame()` is called. The driver is responsible for
    /// ensuring hardware doesn't write to the current buffer.
    fn poll_frame(&self) -> Option<&[u8]>;

    /// Consume the current frame and advance to the next one.
    /// This must be called after processing a frame obtained from `poll_frame()`.
    fn consume_frame(&mut self);

    /// Send a frame on the network.
    fn send_frame(&mut self, frame: &[u8]);
}

#[derive(Error, Debug, Clone, Copy, PartialEq, Eq)]
/// Errors that can occur when using the network stack
pub enum NetworkError {
    #[error("Network controller is not available")]
    /// The network controller is not available
    Absent,
    #[error("Input is invalid")]
    /// The input is invalid
    Invalid,
    #[error("Network controller is not initialized")]
    /// The network controller is not initialized
    Uninitialized,
    #[error("Unsupported operation")]
    /// The operation is not supported
    Unsupported,
}

pub type NetworkResult<T> = Result<T, NetworkError>;
