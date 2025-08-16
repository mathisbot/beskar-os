use num_enum::{FromPrimitive, IntoPrimitive, TryFromPrimitive};

#[derive(Debug, Copy, Clone, PartialEq, Eq, FromPrimitive, IntoPrimitive)]
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
    MemoryMap = 5,
    /// Put the thread to sleep for a given amount of time.
    ///
    /// The first argument is the time to sleep in milliseconds.
    Sleep = 6,
    /// Invalid syscall.
    ///
    /// Any syscall that is not recognized.
    #[num_enum(default)]
    Invalid = u64::MAX,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, FromPrimitive, IntoPrimitive)]
#[repr(u64)]
pub enum SyscallExitCode {
    /// The syscall succeeded
    Success = 0,
    /// The syscall failed
    Failure = 1,

    /// Any other (invalid) exit code.
    #[num_enum(default)]
    Other,
}

impl SyscallExitCode {
    #[track_caller]
    #[inline]
    /// Unwraps the syscall exit code, panicking if it is a failure.
    ///
    /// ## Panics
    ///
    /// Panics if the syscall exit code is not a success.
    pub fn unwrap(self) {
        assert_eq!(
            self,
            Self::Success,
            "Called unwrap on a syscall exit code that was not successful: {self:?}",
        );
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
