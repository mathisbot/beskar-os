use super::traits::{Read, Seek, SeekFrom, Write};
use crate::error::{FileError, FileErrorKind, FileResult, IoError, IoErrorKind, IoResult};
use alloc::string::String;
use beskar_core::syscall::SyscallExitCode;
use core::convert::TryFrom;

type Handle = i64;

#[must_use]
#[inline]
const fn is_valid_handle(handle: Handle) -> bool {
    handle >= 0
}

/// Represents an opened file
pub struct File {
    handle: Handle,
    position: u64,
    path: String,
}

impl File {
    #[expect(clippy::missing_panics_doc, reason = "Never panics")]
    /// Open a file
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be opened
    pub fn open(path: &str) -> FileResult<Self> {
        let handle = crate::sys::sc_open(path.as_ptr(), path.len().try_into().unwrap());
        if is_valid_handle(handle) {
            Ok(Self {
                handle,
                position: 0,
                path: String::from(path),
            })
        } else {
            // TODO: Distinguish error kinds based on syscall error code
            Err(FileError::new(FileErrorKind::Other))
        }
    }

    #[must_use]
    #[inline]
    pub fn path(&self) -> &str {
        &self.path
    }

    #[inline]
    /// Create a file
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be created
    pub fn create(_path: &str) -> FileResult<Self> {
        todo!("Implement when syscalls support file creation")
    }

    #[inline]
    /// Close the file
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be closed
    pub fn close(self) -> FileResult<()> {
        let code = crate::sys::sc_close(self.handle);
        if code == SyscallExitCode::Success {
            Ok(())
        } else {
            Err(FileError::new(FileErrorKind::Other))
        }
    }
}

impl Read for File {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        let n = crate::sys::sc_read(
            self.handle,
            buf.as_mut_ptr(),
            buf.len().try_into().unwrap(),
            self.position,
        );
        if let Ok(n) = usize::try_from(n) {
            self.position += u64::try_from(n).unwrap();
            Ok(n)
        } else {
            Err(IoError::new(IoErrorKind::Other))
        }
    }
}

impl Write for File {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        let n = crate::sys::sc_write(
            self.handle,
            buf.as_ptr(),
            buf.len().try_into().unwrap(),
            self.position,
        );
        if let Ok(n) = usize::try_from(n) {
            self.position += u64::try_from(n).unwrap();
            Ok(n)
        } else {
            Err(IoError::new(IoErrorKind::Other))
        }
    }

    fn flush(&mut self) -> IoResult<()> {
        // No buffering yet, so nothing to flush
        Ok(())
    }
}

impl Seek for File {
    fn seek(&mut self, pos: SeekFrom) -> IoResult<u64> {
        let new_pos = match pos {
            SeekFrom::Start(n) => n,
            SeekFrom::Current(n) => if n >= 0 {
                self.position.checked_add(n.cast_unsigned())
            } else {
                self.position.checked_sub((-n).cast_unsigned())
            }
            .ok_or(IoError::new(IoErrorKind::InvalidData))?,
            SeekFrom::End(_) => return Err(IoError::new(IoErrorKind::Other)), // TODO: Implement when file size syscall is available
        };

        self.position = new_pos;
        Ok(new_pos)
    }
}

impl core::fmt::Write for File {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.write_all(s.as_bytes()).map_err(|_| core::fmt::Error)
    }
}

impl Drop for File {
    #[inline]
    fn drop(&mut self) {
        crate::sys::sc_close(self.handle);
    }
}
