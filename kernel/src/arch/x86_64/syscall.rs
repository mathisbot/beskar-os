use alloc::vec::Vec;
use beskar_core::{
    arch::x86_64::registers::{Efer, LStar, Rflags, SFMask, Star, StarSelectors},
    syscall::Syscall,
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
    // The only reason we cannot enable interrupts here is because we are using
    // a per-core stack for syscall handling.
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
            "pop r11", // r11 contains previous RFLAGS
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
        let mut stack_ptr = unsafe {
            let stack = &mut STACKS[locals!().core_id()];
            stack.as_mut_ptr().add(stack.capacity())
        };

        // Copy arguments to kernel stack
        stack_ptr = unsafe { stack_ptr.byte_sub(size_of::<*mut u8>()) };
        let current_rsp: *mut u8;
        unsafe {
            core::arch::asm!("mov {}, rsp", lateout(reg) current_rsp, options(nomem, nostack, preserves_flags));
        }
        assert!(stack_ptr.cast::<*mut u8>().is_aligned());
        unsafe { stack_ptr.cast::<*mut u8>().write_volatile(current_rsp) };

        stack_ptr = unsafe { stack_ptr.byte_sub(size_of::<SyscallRegisters>()) };
        assert!(stack_ptr.cast::<SyscallRegisters>().is_aligned());
        unsafe {
            stack_ptr
                .cast::<SyscallRegisters>()
                .write_volatile(regs.read());
        }

        #[allow(clippy::pointers_in_nomem_asm_block)] // False positive
        unsafe {
            core::arch::asm!("mov rsp, {}", in(reg) stack_ptr, options(nomem, nostack, preserves_flags));
        }
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
        let current_stack: *mut SyscallRegisters;
        unsafe {
            core::arch::asm!("mov {}, rsp", out(reg) current_stack, options(nomem, nostack, preserves_flags));
        }

        //  Read registers value and previous stack pointer
        let regs = unsafe { current_stack.read() };
        let current_stack = unsafe { current_stack.add(1) };
        let previous_rsp = unsafe { current_stack.cast::<*mut SyscallRegisters>().read() };

        // Write register values back
        unsafe {
            previous_rsp.write_volatile(regs);
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
    LStar::write(syscall_handler_arch);
    Star::write(StarSelectors::new(
        locals!().gdt().kernel_code_selector().0,
        locals!().gdt().kernel_data_selector().0,
        locals!().gdt().user_code_selector().0,
        locals!().gdt().user_data_selector().0,
    ));
    // Disable interrupts on syscall
    // FIXME: Because of this, if a malicious user spams syscalls,
    // it will prevent scheduling of other threads
    unsafe { SFMask::write(Rflags::IF) };

    #[allow(static_mut_refs)]
    unsafe {
        STACKS[locals!().core_id()].reserve(4096 * 8); // 32 KiB
    }

    unsafe { Efer::insert_flags(Efer::SYSTEM_CALL_EXTENSIONS) };
}
