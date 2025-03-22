use ::beskar_core::syscall::Syscall;

#[must_use]
pub fn syscall_1(syscall: Syscall, arg1: u64) -> u64 {
    let res_code: u64;
    unsafe {
        ::core::arch::asm!(
            "syscall",
            in("rax") syscall as u64,
            lateout("rax") res_code,
            in("rdi") arg1,
        );
    }
    res_code
}

#[must_use]
pub fn syscall_2(syscall: Syscall, arg1: u64, arg2: u64) -> u64 {
    let res_code: u64;
    unsafe {
        ::core::arch::asm!(
            "syscall",
            in("rax") syscall as u64,
            lateout("rax") res_code,
            in("rdi") arg1,
            in("rsi") arg2,
        );
    }
    res_code
}

// #[must_use]
// pub fn syscall_3(syscall: Syscall, arg1: u64, arg2: u64, arg3: u64) -> SyscallExitCode {
//     let res_code: u64;
//     unsafe {
//         ::core::arch::asm!(
//             "syscall",
//             in("rax") syscall as u64,
//             lateout("rax") res_code,
//             in("rdi") arg1,
//             in("rsi") arg2,
//             in("rdx") arg3,
//         );
//     }
//     SyscallExitCode::from(res_code)
// }
