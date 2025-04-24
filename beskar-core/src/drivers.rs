use thiserror::Error;

pub mod keyboard;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum DriverError {
    #[error("Device not found")]
    Absent,
    #[error("Invalid device")]
    Invalid,
    #[error("Unknown error")]
    Unknown,
}

pub type DriverResult<T> = Result<T, DriverError>;
