use crate::arch::syscalls;
use beskar_core::syscall::{Syscall, SyscallExitCode};

pub mod keyboard;

#[inline]
/// Print a message to the console
///
/// ## Panics
///
/// Panics if the syscall fails (should never happen
/// for valid input).
pub fn print(msg: &str) {
    let res = syscalls::syscall_2(
        Syscall::Print,
        msg.as_ptr() as u64,
        msg.len().try_into().unwrap(),
    );
    assert_eq!(
        SyscallExitCode::try_from(res).unwrap_or(SyscallExitCode::Other),
        SyscallExitCode::Success
    );
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
#[allow(clippy::missing_panics_doc)] // Never panics
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
#[allow(clippy::missing_panics_doc)] // Never panics
/// Close a file handle
///
/// # Errors
///
/// Returns a `FileError` if the syscall fails.
pub fn close(handle: Handle) -> Result<(), FileError> {
    let res = syscalls::syscall_1(Syscall::Close, handle.cast_unsigned());
    if SyscallExitCode::try_from(res).unwrap_or(SyscallExitCode::Other) == SyscallExitCode::Success
    {
        Ok(())
    } else {
        Err(FileError { code: -1 })
    }
}

#[inline]
#[allow(clippy::missing_panics_doc)] // Never panics
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
        Ok(usize::try_from(res).unwrap_or(0))
    } else {
        Err(FileError { code: res })
    }
}

#[inline]
#[allow(clippy::missing_panics_doc)] // Never panics
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
        Ok(usize::try_from(res).unwrap_or(0))
    } else {
        Err(FileError { code: res })
    }
}
