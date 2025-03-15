use crate::arch::syscalls;
use beskar_core::syscall::{Syscall, SyscallExitCode};

#[inline]
/// Fills the buffer with random bytes
///
/// ## Panics
///
/// Panics if the syscall fails.
/// This will happen if the input data is invalid or if randomness fails to be generated.
pub fn rand(buf: &mut [u8]) {
    let res = syscalls::syscall_2(
        Syscall::RandomGen,
        buf.as_mut_ptr() as u64,
        buf.len().try_into().unwrap(),
    );
    assert_eq!(res, SyscallExitCode::Success);
}
