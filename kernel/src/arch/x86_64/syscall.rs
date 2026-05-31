use crate::{
    locals,
    syscall::{Arguments, syscall},
};
use beskar_core::syscall::{Syscall, SyscallExitCode, SyscallReturnValue};
use beskar_hal::registers::{Efer, LStar, Rflags, SFMask, Star, StarSelectors};

#[derive(Debug, Clone, Copy)]
#[repr(C, align(8))]
/// Represents the pushed registers during a syscall.
///
/// We only push Caller-saved registers, as the others will be saved
/// by the inner syscall handlers.
struct SyscallRegisters {
    rax: u64,
    rdi: u64,
    rsi: u64,
    rdx: u64,
    r10: u64,
    r8: u64,
    r9: u64,
    /// Contains previous value of RIP
    rcx: u64,
    /// Contains previous value of RFLAGS
    r11: u64,
    /// Contains previous value of RSP
    rsp: u64,
}

#[unsafe(naked)]
/// Arch syscall handler, to be loaded into LSTAR.
///
/// # Safety
///
/// This function should not be called directly.
unsafe extern "sysv64" fn syscall_handler_arch() {
    core::arch::naked_asm!(
        "mov gs:[{scratch}], rsp", // Save RSP in GS
        "mov rsp, gs:[{kernel_stack}]", // Swap stack
        "push qword ptr gs:[{scratch}]", // Previous RSP
        "sti",
        "push r11", // Previous RFLAGS
        "push rcx", // Previous RIP
        "push r9",
        "push r8",
        "push r10",
        "push rdx",
        "push rsi",
        "push rdi",
        "push rax",
        "mov rdi, rsp", // Regs pointer
        "call {handler}",
        "pop rax", // RAX now contains syscall exit code
        "pop rdi",
        "pop rsi",
        "pop rdx",
        "pop r10",
        "pop r8",
        "pop r9",
        "pop rcx", // RIP used by sysret
        "pop r11", // r11 contains previous RFLAGS
        "cli",
        "pop rsp", // Restore previous RSP
        "sysretq",
        scratch = const locals::CoreLocalsInfo::scratch_offset(),
        kernel_stack = const locals::CoreLocalsInfo::syscall_stack_offset(),
        handler = sym syscall_handler_inner,
    );
}

/// Performs the standardization of arguments and call to the kernel syscall handler.
///
/// Called by the above function after stack switching
extern "sysv64" fn syscall_handler_inner(regs: &mut SyscallRegisters) {
    let args = Arguments {
        one: regs.rdi,
        two: regs.rsi,
        three: regs.rdx,
        four: regs.r10,
        five: regs.r8,
        six: regs.r9,
    };

    let ssn = Syscall::try_from(regs.rax);

    let res = ssn.map_or(
        SyscallReturnValue::Code(SyscallExitCode::InvalidSyscallNumber),
        |ssn| syscall(ssn, &args),
    );

    // Store result
    regs.rax = res.as_u64();
}

pub fn init_syscalls() {
    LStar::write(syscall_handler_arch);

    locals!().gdt().with_locked(|gdt| {
        Star::write(StarSelectors::new(
            gdt.kernel_code_selector().unwrap(),
            gdt.kernel_data_selector().unwrap(),
            gdt.user_code_selector().unwrap(),
            gdt.user_data_selector().unwrap(),
        ));
    });

    unsafe { SFMask::write(Rflags::IF) };

    unsafe { Efer::insert_flags(Efer::SYSTEM_CALL_EXTENSIONS) };
}
