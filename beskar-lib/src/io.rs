use crate::arch::syscalls;
use beskar_core::syscall::{Syscall, SyscallExitCode};

mod file;
pub use file::File;
pub mod keyboard;

#[inline]
/// Print a message to the console
///
/// # Panics
///
/// Panics if the syscall fails (should never happen
/// for valid input).
pub fn print(msg: &str) {
    const STDOUT_FILE: &str = "/dev/stdout";

    // FIXME: This is very inefficient and faillible if some other process
    // is using stdout. This issue should be handled by the VFS as a special file.
    let file = File::open(STDOUT_FILE).unwrap();

    let bytes_read = file.write(msg.as_bytes(), 0).unwrap();

    file.close().unwrap();

    assert!(bytes_read == msg.len());
}

#[macro_export]
macro_rules! println {
    ($($arg:tt)*) => {
        $crate::io::print(&::alloc::format!($($arg)*));
    };
}

pub type Handle = i64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileError {
    pub code: i64,
}

#[inline]
#[expect(clippy::missing_panics_doc, reason = "Never panics")]
/// Open a file and return its handle
///
/// # Errors
///
/// Returns a `FileError` if the syscall fails.
pub fn open(path: &str) -> Result<Handle, FileError> {
    let raw_res = syscalls::syscall_2(
        Syscall::Open,
        path.as_ptr() as u64,
        path.len().try_into().unwrap(),
    );
    let res = raw_res.cast_signed();
    if res >= 0 {
        Ok(res)
    } else {
        Err(FileError { code: res })
    }
}

#[inline]
/// Close a file handle
///
/// # Errors
///
/// Returns a `FileError` if the syscall fails.
pub fn close(handle: Handle) -> Result<(), FileError> {
    let res = syscalls::syscall_1(Syscall::Close, handle.cast_unsigned());
    if SyscallExitCode::from(res) == SyscallExitCode::Success {
        Ok(())
    } else {
        Err(FileError { code: -1 })
    }
}

#[inline]
#[expect(clippy::missing_panics_doc, reason = "Never panics")]
/// Write a buffer to a file at a given offset
///
/// # Errors
///
/// Returns a `FileError` if the syscall fails.
pub fn write(handle: Handle, buffer: &[u8], offset: usize) -> Result<usize, FileError> {
    let raw_res = syscalls::syscall_4(
        Syscall::Write,
        handle.cast_unsigned(),
        buffer.as_ptr() as u64,
        buffer.len().try_into().unwrap(),
        offset.try_into().unwrap(),
    );
    let res = raw_res.cast_signed();
    if res >= 0 {
        Ok(usize::try_from(res).unwrap())
    } else {
        Err(FileError { code: res })
    }
}

#[inline]
#[expect(clippy::missing_panics_doc, reason = "Never panics")]
/// Read a buffer from a file at a given offset
///
/// # Errors
///
/// Returns a `FileError` if the syscall fails.
pub fn read(handle: Handle, buffer: &mut [u8], offset: usize) -> Result<usize, FileError> {
    let raw_res = syscalls::syscall_4(
        Syscall::Read,
        handle.cast_unsigned(),
        buffer.as_ptr() as u64,
        buffer.len().try_into().unwrap(),
        offset.try_into().unwrap(),
    );
    let res = raw_res.cast_signed();
    if res >= 0 {
        Ok(usize::try_from(res).unwrap())
    } else {
        Err(FileError { code: res })
    }
}
