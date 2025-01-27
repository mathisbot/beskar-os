use alloc::vec::Vec;
use beskar_core::{arch::x86_64::registers::Efer, syscall::Syscall};
use x86_64::{
    VirtAddr,
    registers::{
        model_specific::{LStar, SFMask, Star},
        rflags::RFlags,
    },
};

use crate::{
    locals,
    syscall::{Arguments, syscall},
};

static mut STACKS: [Vec<u8>; 256] = [const { Vec::new() }; 256];

#[derive(Debug, Clone, Copy)]
#[repr(C, align(8))]
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
}

#[naked]
pub(super) unsafe extern "sysv64" fn syscall_handler_arch() {
    unsafe {
        core::arch::naked_asm!(
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
            "call {}",
            "pop rax", // RAX now contains syscall exit code
            "pop rdi",
            "pop rsi",
            "pop rdx",
            "pop r10",
            "pop r8",
            "pop r9",
            "pop rcx", // RIP used by sysret
            "popfq", // r11 contains previous RFLAGS
            "sysretq",
            sym syscall_handler_impl,
        )
    };
}

#[inline]
extern "sysv64" fn syscall_handler_impl(regs: *mut SyscallRegisters) {
    // Switch to kernel stack
    {
        // Get stack location
        #[allow(static_mut_refs)]
        let stack = unsafe { &mut STACKS[locals!().core_id()] };
        let mut stack_ptr = unsafe { stack.as_mut_ptr().add(stack.capacity()) };

        let regs = unsafe { regs.read_volatile() };

        // Copy arguments to kernel stack
        stack_ptr = unsafe { stack_ptr.byte_sub(size_of::<*mut u8>()) };
        let current_rsp: *mut u8;
        unsafe {
            core::arch::asm!("mov {}, rsp", out(reg) current_rsp, options(nomem, nostack, preserves_flags))
        };
        unsafe { stack_ptr.cast::<*mut u8>().write_volatile(current_rsp) };
        stack_ptr = unsafe { stack_ptr.byte_sub(size_of::<SyscallRegisters>()) };
        unsafe { stack_ptr.cast::<SyscallRegisters>().write_volatile(regs) };

        #[allow(clippy::pointers_in_nomem_asm_block)] // False positive
        unsafe {
            core::arch::asm!("mov rsp, {}", in(reg) stack_ptr, options(nomem, nostack, preserves_flags));
        };
    }

    unsafe {
        core::arch::asm!(
        "mov rdi, rsp",
        "call {}",
        sym syscall_handler_inner,
        );
    }

    // Switch back to user stack
    {
        let current_stack: *mut u8;
        unsafe {
            core::arch::asm!("mov {}, rsp", out(reg) current_stack, options(nomem, nostack, preserves_flags));
        }

        //  Read registers value and previous stack pointer
        let regs = unsafe { current_stack.cast::<SyscallRegisters>().read_volatile() };
        let current_sack = unsafe { current_stack.byte_add(size_of::<SyscallRegisters>()) };
        let previous_rsp = unsafe { current_sack.cast::<*mut u8>().read_volatile() };

        // Write register values back
        unsafe {
            previous_rsp.cast::<SyscallRegisters>().write_volatile(regs);
        }

        #[allow(clippy::pointers_in_nomem_asm_block)]
        unsafe {
            core::arch::asm!("mov rsp, {}", in(reg) previous_rsp, options(nomem, nostack, preserves_flags));
        }
    }
}

extern "sysv64" fn syscall_handler_inner(regs: &mut SyscallRegisters) {
    let args = Arguments {
        one: regs.rdi,
        two: regs.rsi,
        three: regs.rdx,
    };

    let res = syscall(Syscall::from(regs.rax), &args);

    // Store result
    regs.rax = res as u64;
}

pub fn init_syscalls() {
    unsafe { Efer::insert_flags(Efer::SYSTEM_CALL_EXTENSIONS) };

    LStar::write(VirtAddr::new(syscall_handler_arch as *const () as u64));
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
        STACKS[locals!().core_id()].reserve(4096 * 16);
    }; // 64 KiB
}
