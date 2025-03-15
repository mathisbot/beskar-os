#[derive(Debug, Copy, Clone, PartialEq, Eq)]
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
    /// Invalid syscall.
    ///
    /// Any syscall that is not recognized.
    Invalid = 0xFF,
}

impl From<u64> for Syscall {
    fn from(value: u64) -> Self {
        match value {
            0 => Self::Print,
            1 => Self::Exit,
            2 => Self::RandomGen,
            _ => Self::Invalid,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SyscallExitCode {
    /// The syscall succeeded
    Success = 0,
    /// The syscall failed
    Failure = 1,

    /// Any other (invalid) exit code.
    Other,
}

impl From<u64> for SyscallExitCode {
    fn from(value: u64) -> Self {
        match value {
            0 => Self::Success,
            1 => Self::Failure,
            _ => Self::Other,
        }
    }
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
pub enum ExitCode {
    Success = 0,
    Failure = 1,
}

impl TryFrom<u64> for ExitCode {
    type Error = ();

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Success),
            1 => Ok(Self::Failure),
            _ => Err(()),
        }
    }
}
