use core::{fmt, result};

pub type IoResult<T> = result::Result<T, IoError>;
pub type FileResult<T> = result::Result<T, FileError>;
pub type MemoryResult<T> = result::Result<T, MemoryError>;
pub type SyscallResult<T> = result::Result<T, SyscallError>;

#[derive(Debug)]
pub struct IoError {
    kind: IoErrorKind,
}

#[derive(Debug, Clone, Copy)]
pub enum IoErrorKind {
    NotFound,
    PermissionDenied,
    InvalidData,
    UnexpectedEof,
    Other,
}

impl IoError {
    #[must_use]
    #[inline]
    pub const fn new(kind: IoErrorKind) -> Self {
        Self { kind }
    }

    #[must_use]
    #[inline]
    pub const fn kind(&self) -> IoErrorKind {
        self.kind
    }
}

#[derive(Debug)]
pub struct FileError {
    kind: FileErrorKind,
}

#[derive(Debug, Clone, Copy)]
pub enum FileErrorKind {
    NotFound,
    PermissionDenied,
    Other,
}

impl FileError {
    #[must_use]
    #[inline]
    pub const fn new(kind: FileErrorKind) -> Self {
        Self { kind }
    }

    #[must_use]
    #[inline]
    pub const fn kind(&self) -> FileErrorKind {
        self.kind
    }
}

#[derive(Debug)]
pub struct MemoryError {
    kind: MemoryErrorKind,
}

impl MemoryError {
    #[must_use]
    #[inline]
    pub const fn new(kind: MemoryErrorKind) -> Self {
        Self { kind }
    }

    #[must_use]
    #[inline]
    pub const fn kind(&self) -> MemoryErrorKind {
        self.kind
    }
}

#[derive(Debug, Clone, Copy)]
pub enum MemoryErrorKind {
    OutOfMemory,
    InvalidAddress,
    PermissionDenied,
    InvalidAlignment,
    Other,
}

#[derive(Debug)]
pub struct SyscallError {
    code: i32,
}

impl SyscallError {
    #[must_use]
    #[inline]
    pub const fn new(code: i32) -> Self {
        Self { code }
    }

    #[must_use]
    #[inline]
    pub const fn code(&self) -> i32 {
        self.code
    }
}

impl fmt::Display for IoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.kind)
    }
}
impl core::error::Error for IoError {}

impl fmt::Display for FileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.kind)
    }
}
impl core::error::Error for FileError {}

impl fmt::Display for MemoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.kind)
    }
}
impl core::error::Error for MemoryError {}

impl fmt::Display for SyscallError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "syscall failed with code {}", self.code)
    }
}
impl core::error::Error for SyscallError {}

#[derive(Debug)]
pub enum Error {
    Io(IoError),
    File(FileError),
    Memory(MemoryError),
    Syscall(SyscallError),
}

pub type Result<T> = result::Result<T, Error>;

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error: {e}"),
            Self::File(e) => write!(f, "File error: {e}"),
            Self::Memory(e) => write!(f, "Memory error: {e}"),
            Self::Syscall(e) => write!(f, "Syscall error: {e}"),
        }
    }
}
impl core::error::Error for Error {}

impl From<IoError> for Error {
    fn from(err: IoError) -> Self {
        Self::Io(err)
    }
}
impl From<FileError> for Error {
    fn from(err: FileError) -> Self {
        Self::File(err)
    }
}
impl From<MemoryError> for Error {
    fn from(err: MemoryError) -> Self {
        Self::Memory(err)
    }
}
impl From<SyscallError> for Error {
    fn from(err: SyscallError) -> Self {
        Self::Syscall(err)
    }
}
