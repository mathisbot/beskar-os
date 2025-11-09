use beskar_core::syscall::Syscall;

pub fn syscall_1(syscall: Syscall, arg1: u64) -> u64 {
    let res_code: u64;
    unsafe {
        core::arch::asm!(
            "syscall",
            in("rax") u64::from(syscall),
            lateout("rax") res_code,
            in("rdi") arg1,
            options(nostack, preserves_flags)
        );
    }
    res_code
}

pub fn syscall_2(syscall: Syscall, arg1: u64, arg2: u64) -> u64 {
    let res_code: u64;
    unsafe {
        core::arch::asm!(
            "syscall",
            in("rax") u64::from(syscall),
            lateout("rax") res_code,
            in("rdi") arg1,
            in("rsi") arg2,
            options(nostack, preserves_flags)
        );
    }
    res_code
}

pub fn syscall_4(syscall: Syscall, arg1: u64, arg2: u64, arg3: u64, arg4: u64) -> u64 {
    let res_code: u64;
    unsafe {
        core::arch::asm!(
            "syscall",
            in("rax") u64::from(syscall),
            lateout("rax") res_code,
            in("rdi") arg1,
            in("rsi") arg2,
            in("rdx") arg3,
            in("r10") arg4,
            options(nostack, preserves_flags)
        );
    }
    res_code
}
