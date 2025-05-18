use super::{FileError, Handle, close, open, read, write};

/// Represents an opened file
pub struct File {
    handle: Handle,
    closed: bool,
}

impl File {
    #[must_use]
    #[inline]
    const fn new(handle: Handle) -> Self {
        Self {
            handle,
            closed: false,
        }
    }

    #[must_use]
    #[inline]
    pub const fn handle(&self) -> Handle {
        self.handle
    }

    #[inline]
    /// Open a file
    ///
    /// # Errors
    ///
    /// Returns a `FileError` if the syscall fails.
    pub fn open(path: &str) -> Result<Self, FileError> {
        Ok(Self::new(open(path)?))
    }

    #[inline]
    /// Read a buffer from the file at a given offset
    ///
    /// # Errors
    ///
    /// Returns a `FileError` if the syscall fails.
    pub fn read(&self, buffer: &mut [u8], offset: usize) -> Result<usize, FileError> {
        read(self.handle, buffer, offset)
    }

    #[inline]
    /// Write a buffer to the file at a given offset
    ///
    /// # Errors
    ///
    /// Returns a `FileError` if the syscall fails.
    pub fn write(&self, buffer: &[u8], offset: usize) -> Result<usize, FileError> {
        write(self.handle, buffer, offset)
    }

    #[inline]
    /// Close the file
    ///
    /// # Errors
    ///
    /// Returns a `FileError` if the syscall fails.
    pub fn close(mut self) -> Result<(), FileError> {
        if self.closed {
            return Ok(());
        }

        let close_res = close(self.handle);
        self.closed = close_res.is_ok();
        close_res
    }
}

impl Drop for File {
    #[inline]
    fn drop(&mut self) {
        if !self.closed {
            let _ = close(self.handle);
        }
    }
}
