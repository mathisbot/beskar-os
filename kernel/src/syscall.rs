pub enum Syscall {
    /// Print syscall.
    ///
    /// The first argument is a pointer to the string to print.
    /// The second argument is the length of the string.
    Print,
    Invalid = 0xFF,
}

impl From<usize> for Syscall {
    fn from(value: usize) -> Self {
        match value {
            0 => Self::Print,
            _ => Self::Invalid,
        }
    }
}

pub struct Arguments {
    pub one: usize,
    pub two: usize,
    pub three: usize,
}

#[repr(u8)]
pub enum SyscallExitCode {
    Success,
    Failure,
}

pub fn syscall(syscall: Syscall, args: Arguments) -> SyscallExitCode {
    match syscall {
        Syscall::Print => {
            let string = unsafe {
                core::str::from_utf8_unchecked(core::slice::from_raw_parts(
                    args.one as *const u8,
                    args.two,
                ))
            };
            crate::info!("{}", string);
            SyscallExitCode::Success
        }
        Syscall::Invalid => SyscallExitCode::Failure,
    }
}
