use num_enum::{IntoPrimitive, TryFromPrimitive};

#[derive(Debug, Copy, Clone, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
#[repr(u64)]
pub enum Syscall {
    /// Exit syscall.
    ///
    /// The first argument is the exit code.
    Exit = 0,
    /// Open syscall.
    ///
    /// Opens a file from the filesystem and returns a handle to it.
    ///
    /// The first argument is a pointer to the file path.
    /// The second argument is the length of the path.
    Open = 1,
    /// Close syscall.
    ///
    /// Closes a file handle.
    ///
    /// The first argument is the handle to the file.
    Close = 2,
    /// Read syscall.
    ///
    /// Reads a file from the filesystem.
    ///
    /// The first argument is a handle to the file.
    /// The second argument is a pointer to the buffer to read into.
    /// The third argument is the length of the buffer.
    /// The fourth argument is the offset to read from.
    Read = 3,
    /// Write syscall.
    ///
    /// Writes to a file from the filesystem.
    ///
    /// The first argument is a handle to the file.
    /// The second argument is a pointer to the buffer to write from.
    /// The third argument is the length of the buffer.
    /// The fourth argument is the offset to write to.
    Write = 4,
    /// MemoryMap syscall.
    ///
    /// Allocates memory and gives the user a pointer to it.
    ///
    /// The first argument is the size of the memory to allocate.
    /// The second argument is the alignment of the memory.
    /// The third argument is the protection flags of the memory.
    MemoryMap = 5,
    /// MemoryUnmap syscall.
    ///
    /// Frees previously allocated memory.
    ///
    /// The first argument is the pointer to the memory region.
    /// The second argument is the size of the memory region.
    MemoryUnmap = 6,
    /// MemoryProtect syscall.
    ///
    /// Changes the protection of a memory region.
    ///
    /// The first argument is the pointer to the memory region.
    /// The second argument is the size of the memory region.
    /// The third argument is the new protection flags.
    MemoryProtect = 7,
    /// Put the thread to sleep for a given amount of time.
    ///
    /// The first argument is the time to sleep in milliseconds.
    Sleep = 8,
    /// Put the thread to sleep until a given event is signalled.
    ///
    /// The first argument is the sleep handle to wait on.
    WaitOnEvent = 9,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
#[repr(u64)]
pub enum SyscallExitCode {
    /// The syscall succeeded
    Success = 0,
    /// The syscall failed
    Failure = 1,
    /// The syscall number was invalid
    InvalidSyscallNumber = 2,
}

impl SyscallExitCode {
    #[track_caller]
    #[inline]
    /// Unwraps the syscall exit code, panicking if it is a failure.
    ///
    /// # Panics
    ///
    /// Panics if the syscall exit code is not a success.
    pub fn unwrap(self) {
        assert_eq!(
            self,
            Self::Success,
            "Called unwrap on a syscall exit code that was not successful: {self:?}",
        );
    }

    #[must_use]
    #[inline]
    pub const fn is_success(self) -> bool {
        matches!(self, Self::Success)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SyscallReturnValue {
    Code(SyscallExitCode),
    ValueU(u64),
    ValueI(i64),
}

impl SyscallReturnValue {
    #[must_use]
    #[inline]
    pub const fn as_u64(self) -> u64 {
        match self {
            Self::Code(code) => code as u64,
            Self::ValueU(value) => value,
            Self::ValueI(value) => value.cast_unsigned(),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
#[repr(u64)]
pub enum ExitCode {
    Success = 0,
    Failure = 1,
}

/// Syscall-related constants
pub mod consts {
    /// Memory protection flags - read permission
    pub const MFLAGS_READ: u64 = 0x1;
    /// Memory protection flags - write permission
    pub const MFLAGS_WRITE: u64 = 0x2;
    /// Memory protection flags - execute permission
    pub const MFLAGS_EXECUTE: u64 = 0x4;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_syscall_exit_code_unwrap() {
        SyscallExitCode::Success.unwrap();
    }

    #[test]
    #[should_panic(
        expected = "Called unwrap on a syscall exit code that was not successful: Failure"
    )]
    fn test_syscall_exit_code_unwrap_failure() {
        SyscallExitCode::Failure.unwrap();
    }
}
