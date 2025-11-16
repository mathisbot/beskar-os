use alloc::{string::String, vec::Vec};
use thiserror::Error;

pub mod dev;
pub mod ext2;
pub mod fat;
pub mod in_mem;

#[derive(Debug, Error, Clone, Copy, Eq, PartialEq)]
pub enum FileError {
    #[error("I/O error")]
    Io,
    #[error("File not found")]
    NotFound,
    #[error("Invalid path")]
    InvalidPath,
    #[error("Invalid handle")]
    InvalidHandle,
    #[error("File already exists")]
    AlreadyExists,
    #[error("File system is full")]
    NotEnoughSpace,
    #[error("Unexpected end of file")]
    UnexpectedEof,
    #[error("Permission denied")]
    PermissionDenied,
    #[error("File system is corrupted")]
    CorruptedFS,
    #[error("Unsupported operation")]
    UnsupportedOperation,
}

impl From<super::BlockDeviceError> for FileError {
    fn from(error: super::BlockDeviceError) -> Self {
        match error {
            super::BlockDeviceError::Io | super::BlockDeviceError::UnalignedAccess => Self::Io,
            super::BlockDeviceError::OutOfBounds => Self::UnexpectedEof,
            super::BlockDeviceError::Unsupported => Self::UnsupportedOperation,
        }
    }
}

pub type FileResult<T> = Result<T, FileError>;

/// A trait representing a file system interface.
///
/// This trait defines the basic operations that can be performed on a file system,
/// such as creating, deleting, opening, and reading files.
/// It is designed to be implemented by different file system types,
/// allowing for a uniform interface to interact with various file systems.
///
/// # Notes
///
/// Some checks are already performed by the VFS layer, such as `Handle` validity.
/// The FS layer does not need to check them again.
pub trait FileSystem {
    /// Creates a new file at the given path, if it does not already exist.
    fn create(&mut self, path: Path) -> FileResult<()>;
    /// Deletes the file at the given path.
    fn delete(&mut self, path: Path) -> FileResult<()>;
    /// Checks if a file exists at the given path.
    fn exists(&mut self, path: Path) -> FileResult<bool>;
    /// Opens the file at the given path and returns a handle to it.
    ///
    /// This can be a no-op for some filesystems
    /// (file handles are handled by the VFS layer).
    fn open(&mut self, path: Path) -> FileResult<()>;
    /// Closes the file.
    ///
    /// This can be a no-op for some filesystems
    /// (file handles are handled by the VFS layer).
    fn close(&mut self, path: Path) -> FileResult<()>;
    /// Reads from the file associated with the given handle into the given buffer.
    ///
    /// This returns how many bytes were read.
    fn read(&mut self, path: Path, buffer: &mut [u8], offset: usize) -> FileResult<usize>;
    /// Writes the given buffer to the file associated with the given handle.
    ///
    /// This returns how many bytes were written.
    fn write(&mut self, path: Path, buffer: &[u8], offset: usize) -> FileResult<usize>;
    /// Returns information about the file at the given path.
    fn metadata(&mut self, path: Path) -> FileResult<FileMetadata>;
    /// Returns every entry in the directory at the given path.
    fn read_dir(&mut self, path: Path) -> FileResult<Vec<PathBuf>>;
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct PathBuf(String);

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub struct Path<'a>(&'a str);

impl PathBuf {
    #[must_use]
    #[inline]
    /// Creates a new `PathBuf` from the given string.
    pub fn new(path: &str) -> Self {
        Self(String::from(path))
    }

    #[inline]
    #[doc(alias = "push_str")]
    /// Pushes a new path to the current path.
    pub fn push(&mut self, path: &str) {
        self.0.push_str(path);
    }

    #[must_use]
    #[inline]
    pub fn as_path(&self) -> Path<'_> {
        Path(&self.0)
    }

    #[inline]
    pub fn join(&self, path: &str) -> PathBuf {
        let mut new_path = self.0.clone();
        new_path.push_str(path);
        PathBuf(new_path)
    }
}

impl core::borrow::Borrow<str> for PathBuf {
    #[inline]
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl<'a> Path<'a> {
    #[must_use]
    #[inline]
    /// Creates a new `Path` from the given string slice.
    pub const fn new(path: &'a str) -> Self {
        Self(path)
    }
}

impl Path<'_> {
    #[must_use]
    #[inline]
    /// Allocates a new `PathBuf` from the current path.
    pub fn to_owned(&self) -> PathBuf {
        PathBuf::new(self.0)
    }

    #[must_use]
    #[inline]
    pub const fn as_str(&self) -> &str {
        self.0
    }
}

impl<'a> From<&'a str> for Path<'a> {
    #[inline]
    fn from(value: &'a str) -> Self {
        Self(value)
    }
}

impl core::ops::Deref for Path<'_> {
    type Target = str;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.0
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum FileType {
    File,
    Directory,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct FileMetadata {
    size: usize,
    file_type: FileType,
}

impl FileMetadata {
    #[must_use]
    #[inline]
    pub const fn new(size: usize, file_type: FileType) -> Self {
        Self { size, file_type }
    }

    #[must_use]
    #[inline]
    pub const fn size(&self) -> usize {
        self.size
    }

    #[must_use]
    #[inline]
    pub const fn file_type(&self) -> FileType {
        self.file_type
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pathbuf() {
        let mut path = PathBuf::new("/home/user");
        path.push("/documents");
        assert_eq!(path.as_path().as_str(), "/home/user/documents");
    }
}
