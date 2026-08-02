use beskar_core::syscall::Syscall;

pub fn syscall_0(syscall: Syscall) -> u64 {
    let res_code: u64;
    unsafe {
        core::arch::asm!(
            "syscall",
            in("rax") u64::from(syscall),
            lateout("rax") res_code,
            lateout("rcx") _,
            lateout("r11") _,
            options(nostack, preserves_flags)
        );
    }
    res_code
}

pub fn syscall_1(syscall: Syscall, arg1: u64) -> u64 {
    let res_code: u64;
    unsafe {
        core::arch::asm!(
            "syscall",
            in("rax") u64::from(syscall),
            lateout("rax") res_code,
            in("rdi") arg1,
            lateout("rcx") _,
            lateout("r11") _,
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
            lateout("rcx") _,
            lateout("r11") _,
            options(nostack, preserves_flags)
        );
    }
    res_code
}

pub fn syscall_3(syscall: Syscall, arg1: u64, arg2: u64, arg3: u64) -> u64 {
    let res_code: u64;
    unsafe {
        core::arch::asm!(
            "syscall",
            in("rax") u64::from(syscall),
            lateout("rax") res_code,
            in("rdi") arg1,
            in("rsi") arg2,
            in("rdx") arg3,
            lateout("rcx") _,
            lateout("r11") _,
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
            lateout("rcx") _,
            lateout("r11") _,
            options(nostack, preserves_flags)
        );
    }
    res_code
}
