#![no_std]
#![forbid(unsafe_op_in_unsafe_fn)]
#![warn(clippy::pedantic, clippy::nursery)]
//! Holonet is the galactic network stack for the kernel.

extern crate alloc;
use thiserror::Error;

pub mod l2;
pub mod l3;
pub mod l4;
mod utils;

pub trait Nic {
    fn mac_address(&self) -> crate::l2::ethernet::MacAddress;
    fn poll_frame(&self) -> Option<&[u8]>;
    fn send_frame(&self, frame: &[u8]);
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
