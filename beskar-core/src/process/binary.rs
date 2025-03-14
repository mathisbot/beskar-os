use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum LoadError {
    #[error("Invalid binary")]
    InvalidBinary,
}

pub type BinaryResult<T> = Result<T, LoadError>;
