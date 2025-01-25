use alloc::vec::Vec;
use x86_64::{
    VirtAddr,
    registers::{
        model_specific::{LStar, SFMask, Star},
        rflags::RFlags,
    },
};

use crate::{
    locals,
    syscall::{Arguments, Syscall, syscall},
};

static mut STACKS: [Vec<u8>; 256] = [const { Vec::new() }; 256];

#[derive(Debug, Clone, Copy)]
struct SyscallRegisters {
    rax: u64,
    rdi: u64,
    rsi: u64,
    rdx: u64,
    r10: u64,
    r8: u64,
    r9: u64,
    /// Contains preivous value of RIP
    rcx: u64,
    /// Contains previous value of RFLAGS
    r11: u64,
    rsp: u64,
}

impl SyscallRegisters {
    #[must_use]
    #[inline]
    pub unsafe fn load() -> Self {
        let rax: u64;
        let rdi: u64;
        let rsi: u64;
        let rdx: u64;
        let r10: u64;
        let r8: u64;
        let r9: u64;
        let rcx: u64;
        let r11: u64;
        let rsp: u64;

        unsafe {
            core::arch::asm!(
                "mov r12, rsp",
                out("rax") rax,
                out("rdi") rdi,
                out("rsi") rsi,
                out("rdx") rdx,
                out("r10") r10,
                out("r8") r8,
                out("r9") r9,
                out("rcx") rcx,
                out("r11") r11,
                out("r12") rsp,
                options(nomem, nostack, preserves_flags)
            )
        };

        Self {
            rax,
            rdi,
            rsi,
            rdx,
            r10,
            r8,
            r9,
            rcx,
            r11,
            rsp,
        }
    }

    #[inline]
    pub unsafe fn store(&self) {
        unsafe {
            core::arch::asm!(
                "mov rsp, r12",
                in("rax") self.rax,
                in("rdi") self.rdi,
                in("rsi") self.rsi,
                in("rdx") self.rdx,
                in("r10") self.r10,
                in("r8") self.r8,
                in("r9") self.r9,
                in("rcx") self.rcx,
                in("r11") self.r11,
                in("r12") self.rsp,
                options(nomem, nostack, preserves_flags)
            )
        };
    }
}

#[inline]
pub(super) extern "sysv64" fn syscall_handler_impl() {
    let mut regs = unsafe { SyscallRegisters::load() };

    // Switch to kernel stack
    #[allow(static_mut_refs)]
    let stack = unsafe { &mut STACKS[locals!().core_id()] }.as_mut_ptr();
    unsafe {
        core::arch::asm!("mov rsp, {}", in(reg) stack, options(nomem, nostack, preserves_flags))
    };

    let args = Arguments {
        one: regs.rdi,
        two: regs.rsi,
        three: regs.rdx,
    };

    // Save meaningful values
    let rcx = regs.rcx;
    let r11 = regs.r11;

    let res = syscall(Syscall::from(regs.rax), args);

    // Restore meaningful values
    regs.rax = res as u64;
    regs.rcx = rcx;
    regs.r11 = r11;

    unsafe { regs.store() };

    if r11 & RFlags::INTERRUPT_FLAG.bits() == 0 {
        return;
    }

    unsafe { core::arch::asm!("sti", "sysret",) };
}

pub fn init_syscalls() {
    LStar::write(VirtAddr::new(syscall_handler_impl as *const () as u64));
    Star::write(
        locals!().gdt().user_code_selector(),
        locals!().gdt().user_data_selector(),
        locals!().gdt().kernel_code_selector(),
        locals!().gdt().kernel_data_selector(),
    )
    .unwrap();
    // Disable interrupts on syscall
    // FIXME: Because of this, if a malicious user spams syscalls,
    // it will prevent scheduling of other threads
    SFMask::write(RFlags::INTERRUPT_FLAG);

    #[allow(static_mut_refs)]
    unsafe {
        STACKS[locals!().core_id()].reserve(4096 * 4)
    }; // 16 KiB
}
