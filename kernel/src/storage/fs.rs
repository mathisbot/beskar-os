use alloc::string::String;
use thiserror::Error;

pub mod ext2;
pub mod fat32;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct Handle(pub usize);

#[derive(Debug, Error)]
pub enum Error {
    #[error("File not found")]
    NotFound,
    #[error("File already exists")]
    AlreadyExists,
    #[error("Invalid path")]
    InvalidPath,
    #[error("Invalid handle")]
    InvalidHandle,
    #[error("IO error")]
    IoError,
}

type FileResult<T> = Result<T, Error>;

pub trait FileSystem {
    /// Creates a new file at the given path, if it does not already exist.
    fn create(&self, path: &str) -> FileResult<Handle>;
    /// Deletes the file at the given path.
    fn delete(&self, path: &str) -> FileResult<()>;
    /// Checks if a file exists at the given path.
    fn exists(&self, path: &str) -> FileResult<bool>;
    /// Opens the file at the given path and returns a handle to it.
    fn open(&self, path: &str) -> FileResult<Handle>;
    /// Closes the file associated with the given handle.
    fn close(&self, handle: Handle) -> FileResult<()>;
    /// Reads from the file associated with the given handle into the given buffer.
    ///
    /// This returns how many bytes were read.
    fn read(&self, handle: Handle, buffer: &mut [u8], offset: usize) -> FileResult<usize>;
    /// Writes the given buffer to the file associated with the given handle.
    ///
    /// This returns how many bytes were written.
    fn write(&self, path: &str, buffer: &[u8]) -> FileResult<usize>;
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct PathBuf(String);

pub struct Path<'a>(&'a str);

impl PathBuf {
    #[must_use]
    #[inline]
    pub fn new(path: &str) -> Self {
        Self(String::from(path))
    }

    #[must_use]
    #[inline]
    pub fn push(&mut self, path: &str) {
        self.0.push_str(path);
    }

    #[must_use]
    #[inline]
    pub fn as_path(&self) -> Path {
        Path(&self.0)
    }
}

impl<'a> From<&'a str> for Path<'a> {
    #[inline]
    fn from(value: &'a str) -> Self {
        Self(value)
    }
}
