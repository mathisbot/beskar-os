pub fn init() {
    crate::arch::syscall::init_syscalls();
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Syscall {
    /// Print syscall.
    ///
    /// The first argument is a pointer to the string to print.
    /// The second argument is the length of the string.
    Print,
    Exit,
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

pub struct Arguments {
    pub one: u64,
    pub two: u64,
    pub three: u64,
}

#[repr(u8)]
pub enum SyscallExitCode {
    Success,
    Failure,
}

pub fn syscall(syscall: Syscall, args: &Arguments) -> SyscallExitCode {
    match syscall {
        Syscall::Print => {
            let string = unsafe {
                core::str::from_utf8_unchecked(core::slice::from_raw_parts(
                    args.one as *const u8,
                    usize::try_from(args.two).unwrap(),
                ))
            };
            crate::info!("{}", string);
            SyscallExitCode::Success
        }
        Syscall::Exit => {
            // TODO: Use exit code in `args.one`
            unsafe { crate::process::scheduler::exit_current_thread() };
            SyscallExitCode::Success
        }
        Syscall::Invalid => SyscallExitCode::Failure,
    }
}
