/// Errors that can occur during heap operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeapError {
    /// Out of memory
    OutOfMemory,
    /// Invalid alignment (must be power of 2)
    InvalidAlignment,
    /// Invalid size (must be non-zero)
    InvalidSize,
    /// Invalid layout
    InvalidLayout,
    /// Attempted to free a pointer that was not allocated
    InvalidPointer,
    /// Double free detected
    DoubleFree,
}

impl core::fmt::Display for HeapError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::OutOfMemory => write!(f, "Out of memory"),
            Self::InvalidAlignment => write!(f, "Invalid alignment (must be power of 2)"),
            Self::InvalidSize => write!(f, "Invalid size (must be non-zero)"),
            Self::InvalidLayout => write!(f, "Invalid layout"),
            Self::InvalidPointer => write!(f, "Invalid pointer"),
            Self::DoubleFree => write!(f, "Double free detected"),
        }
    }
}
impl core::error::Error for HeapError {}

pub type Result<T> = core::result::Result<T, HeapError>;
