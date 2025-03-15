use crate::arch::syscalls;
use beskar_core::syscall::{Syscall, SyscallExitCode};

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
    assert_eq!(res, SyscallExitCode::Success);
}
