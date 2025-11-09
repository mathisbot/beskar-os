use super::traits::{Read, Seek, SeekFrom, Write};
use crate::{
    arch::syscalls,
    error::{FileError, FileErrorKind, FileResult, IoError, IoErrorKind, IoResult},
};
use alloc::string::String;
use beskar_core::syscall::{Syscall, SyscallExitCode};
use core::convert::TryFrom;

type Handle = i64;

/// Represents an opened file
pub struct File {
    handle: Handle,
    position: u64,
    path: String,
}

impl File {
    #[must_use]
    #[inline]
    const fn new(handle: Handle, path: String) -> Self {
        Self {
            handle,
            position: 0,
            path,
        }
    }

    #[must_use]
    #[inline]
    pub fn path(&self) -> &str {
        &self.path
    }

    #[expect(clippy::missing_panics_doc, reason = "Never panics")]
    /// Open a file
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be opened
    pub fn open(path: &str) -> FileResult<Self> {
        let raw_res = syscalls::syscall_2(
            Syscall::Open,
            path.as_ptr() as u64,
            path.len().try_into().unwrap(),
        );
        let handle = raw_res.cast_signed();

        if handle >= 0 {
            Ok(Self::new(handle, String::from(path)))
        } else {
            // TODO: Distinguish error kinds based on syscall error code
            Err(FileError::new(FileErrorKind::Other))
        }
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
        let res = syscalls::syscall_1(Syscall::Close, self.handle.cast_unsigned());
        if SyscallExitCode::from(res) == SyscallExitCode::Success {
            Ok(())
        } else {
            Err(FileError::new(FileErrorKind::Other))
        }
    }
}

impl Read for File {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        let raw_res = syscalls::syscall_4(
            Syscall::Read,
            self.handle.cast_unsigned(),
            buf.as_ptr() as u64,
            buf.len().try_into().unwrap(),
            self.position,
        );
        let n = raw_res.cast_signed();

        if n >= 0 {
            let n = usize::try_from(n).unwrap();
            self.position += u64::try_from(n).unwrap();
            Ok(n)
        } else {
            Err(IoError::new(IoErrorKind::Other))
        }
    }
}

impl Write for File {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        let raw_res = syscalls::syscall_4(
            Syscall::Write,
            self.handle.cast_unsigned(),
            buf.as_ptr() as u64,
            buf.len().try_into().unwrap(),
            self.position,
        );
        let n = raw_res.cast_signed();

        if n >= 0 {
            let n = usize::try_from(n).unwrap();
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
        let _ = syscalls::syscall_1(Syscall::Close, self.handle.cast_unsigned());
    }
}
