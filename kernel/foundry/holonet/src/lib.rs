//! Holonet is the galactic network stack for the kernel.
#![no_std]
#![allow(clippy::double_must_use, clippy::missing_errors_doc)]

extern crate alloc;

use thiserror::Error;

pub mod egress;
pub mod ingress;
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
    #[error("Input is malformed")]
    /// The input is malformed
    Invalid,
    #[error("Buffer is too short")]
    /// The provided buffer is too short for the requested operation
    Truncated,
    #[error("Network controller is not initialized")]
    /// The network controller is not initialized
    Uninitialized,
    #[error("Unsupported operation")]
    /// The operation is not supported
    Unsupported,
    #[error("Destination is unreachable")]
    /// No route or neighbor path is currently available for the destination
    Unreachable,
    #[error("Value exceeds protocol limits")]
    /// The requested operation exceeds the protocol field width
    Oversized,
    #[error("Resources exhausted")]
    /// All available resources of this type are in use
    Exhausted,
    #[error("Resource already exists")]
    /// A conflicting resource already exists
    AlreadyExists,
}

pub type NetworkResult<T> = Result<T, NetworkError>;
