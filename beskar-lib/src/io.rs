use beskar_core::syscall::{Syscall, SyscallExitCode};

/// Print a message to the console
///
/// ## Panics
///
/// Panics if the syscall fails (should never happen)
pub fn print(msg: &str) {
    let res_code: u64;
    unsafe {
        core::arch::asm!(
            "syscall",
            in("rax") Syscall::Print as u64,
            lateout("rax") res_code,
            in("rdi") msg.as_ptr(),
            in("rsi") msg.len(),
        );
    }
    assert_eq!(res_code, SyscallExitCode::Success as u64);
}
