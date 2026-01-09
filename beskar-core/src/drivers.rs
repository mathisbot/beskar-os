use thiserror::Error;

pub mod keyboard;

/// Errors that can occur during block device operations.
///
/// These errors represent the main categories of block device failures,
/// with internal context that can be examined for debugging or logging.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum DriverError {
    /// Device is not present or available
    #[error("Device not found")]
    Absent,
    /// Device is present but invalid or in an invalid state
    #[error("Invalid device")]
    Invalid,
    /// An unknown error occurred
    #[error("Unknown error")]
    Unknown,
}

pub type DriverResult<T> = Result<T, DriverError>;
