#![no_std]
#![forbid(unsafe_op_in_unsafe_fn)]
#![warn(clippy::pedantic, clippy::nursery)]

extern crate alloc;
use thiserror::Error;

pub mod l2;
pub mod l3;
pub mod l4;

pub trait Nic {
    fn mac_address(&self) -> crate::l2::ethernet::MacAddress;
    fn poll_frame(&self) -> Option<&[u8]>;
    fn send_frame(&self, frame: &[u8]);
}

#[derive(Error, Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkError {
    #[error("Network controller is not available")]
    /// The network controller is not available
    Absent,
    #[error("Network controller is not supported")]
    /// The network controller is not supported
    Invalid,
    #[error("Network controller is not initialized")]
    /// The network controller is not initialized
    Uninitialized,
    #[error("Network controller is already initialized")]
    /// The network controller is already initialized
    AlreadyInitialized,
}

pub type NetworkResult<T> = Result<T, NetworkError>;
