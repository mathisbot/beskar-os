use num_enum::{IntoPrimitive, TryFromPrimitive};

#[derive(Debug, Copy, Clone, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
#[repr(u64)]
pub enum Syscall {
    /// Print syscall.
    ///
    /// The first argument is a pointer to the string to print.
    /// The second argument is the length of the string.
    Print = 0,
    /// Exit syscall.
    ///
    /// The first argument is the exit code.
    Exit = 1,
    /// RandomGen syscall.
    ///
    /// Fills a given buffer with random data.
    ///
    /// The first argument is a pointer to the buffer.
    /// The second argument is the length of the buffer.
    RandomGen = 2,
    /// MemoryMap syscall.
    ///
    /// Allocates memory and gives the user a pointer to it.
    ///
    /// The first argument is the size of the memory to allocate.
    MemoryMap = 3,
    // FIXME: When VFS is working, this syscall should be fused with a file read syscall.
    /// KeybooardPoll syscall.
    ///
    /// Polls the keyboard for input.
    KeyboardPoll = 4,
    /// Invalid syscall.
    ///
    /// Any syscall that is not recognized.
    Invalid = u64::MAX,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
#[repr(u64)]
pub enum SyscallExitCode {
    /// The syscall succeeded
    Success = 0,
    /// The syscall failed
    Failure = 1,

    /// Any other (invalid) exit code.
    Other,
}

impl SyscallExitCode {
    #[inline]
    /// Unwraps the syscall exit code, panicking if it is a failure.
    ///
    /// ## Panics
    ///
    /// Panics if the syscall exit code is not a success.
    pub fn unwrap(self) {
        assert_ne!(self, Self::Failure, "Syscall failed!");
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SyscallReturnValue {
    Code(SyscallExitCode),
    Value(u64),
}

impl SyscallReturnValue {
    #[must_use]
    #[inline]
    pub const fn as_u64(self) -> u64 {
        match self {
            Self::Code(code) => code as u64,
            Self::Value(value) => value,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
#[repr(u64)]
pub enum ExitCode {
    Success = 0,
    Failure = 1,
}
