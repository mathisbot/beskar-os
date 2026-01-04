//! Error types for ELF binary loading.

/// Errors that can occur during ELF binary loading.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElfLoadError {
    /// Invalid ELF header or format
    InvalidBinary,
    /// Unsupported ELF feature
    UnsupportedFeature,
    /// Memory mapping failed
    MapperError,
    /// Relocation error
    RelocationError,
    /// Invalid segment
    InvalidSegment,
    /// Arithmetic overflow
    Overflow,
}

impl core::fmt::Display for ElfLoadError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidBinary => write!(f, "invalid ELF binary"),
            Self::UnsupportedFeature => write!(f, "unsupported ELF feature"),
            Self::MapperError => write!(f, "memory mapper error"),
            Self::RelocationError => write!(f, "relocation error"),
            Self::InvalidSegment => write!(f, "invalid segment"),
            Self::Overflow => write!(f, "arithmetic overflow"),
        }
    }
}
