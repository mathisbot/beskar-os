#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Syscall {
    /// Print syscall.
    ///
    /// The first argument is a pointer to the string to print.
    /// The second argument is the length of the string.
    Print = 0,
    Exit = 1,
    Invalid = 0xFF,
}

impl From<u64> for Syscall {
    fn from(value: u64) -> Self {
        match value {
            0 => Self::Print,
            1 => Self::Exit,
            _ => Self::Invalid,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SyscallExitCode {
    Success = 0,
    Failure = 1,
}

impl SyscallExitCode {
    pub fn unwrap(self) {
        if self == Self::Failure {
            panic!("Syscall failed!");
        }
    }
}
