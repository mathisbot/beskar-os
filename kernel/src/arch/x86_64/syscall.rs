use alloc::vec::Vec;
use beskar_core::{
    arch::{
        commons::VirtAddr,
        x86_64::registers::{Efer, LStar, Rflags, SFMask, Star, StarSelectors},
    },
    syscall::Syscall,
};
use hyperdrive::once::Once;

use crate::{
    locals,
    syscall::{Arguments, syscall},
};

// FIXME: Determine the number of stacks dynamically
static STACKS: [Once<Vec<u8>>; 256] = [const { Once::uninit() }; 256];

const SYSCALL_STACK_SIZE: usize = 4096 * 8; // 32 KiB

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
/// Arch syscall handler, to be loaded into LSTAR.
///
/// ## Safety
///
/// This function should not be called directly.
unsafe extern "sysv64" fn syscall_handler_arch() {
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

/// Handles stack switching and calling the actual syscall handler.
///
/// This function is called from the assembly stub above.
extern "sysv64" fn syscall_handler_impl(regs: *mut SyscallRegisters) {
    // Currently, we are on the user stack. It is undefined wether we are right where the
    // assembly stub left us (because of Rust), but the place we want to be is in the `regs` argument.

    // Perform the stack switch to kernel stack
    {
        // Make sure we have a nice aligned stack pointer
        let mut stack_ptr = {
            // Note that we do not lock the stack here, as it is a per-core stack
            // and that interrupts (scheduling) are disabled during syscall handling.
            // Locking the stack for the entire duration of the syscall handling would
            // be a bad idea, as panicking/exiting threads would poison the lock.
            let kernel_stack = STACKS[locals!().core_id()].get().unwrap();
            let stack_bottom = kernel_stack.as_ptr();
            let stack_top_unaligned = unsafe { stack_bottom.add(kernel_stack.capacity()) };
            let stack_top_vaddr = VirtAddr::new(stack_top_unaligned as u64).align_down(16);
            stack_top_vaddr.as_mut_ptr::<u8>()
        };

        // Copy current RSP to kernel stack
        #[allow(clippy::cast_ptr_alignment)] // `stack_ptr` is 16-byte aligned
        {
            let user_rsp: *mut u8;
            unsafe {
                core::arch::asm!("mov {}, rsp", lateout(reg) user_rsp, options(nomem, nostack, preserves_flags));
            }
            stack_ptr = unsafe { stack_ptr.byte_sub(size_of::<*mut u8>()) };
            debug_assert!(stack_ptr.cast::<*mut u8>().is_aligned());
            unsafe { stack_ptr.cast::<*mut u8>().write_volatile(user_rsp) };
        }

        // Copy pointer to `regs` to kernel stack
        #[allow(clippy::cast_ptr_alignment)] // `stack_ptr` is 8-byte aligned
        {
            stack_ptr = unsafe { stack_ptr.byte_sub(size_of::<*mut SyscallRegisters>()) };
            debug_assert!(stack_ptr.cast::<*mut SyscallRegisters>().is_aligned());
            unsafe {
                stack_ptr
                    .cast::<*mut SyscallRegisters>()
                    .write_volatile(regs);
            }
        }

        // Finaly, switch to kernel stack
        #[allow(clippy::pointers_in_nomem_asm_block)] // False positive
        unsafe {
            core::arch::asm!("mov rsp, {}", in(reg) stack_ptr, options(nomem, nostack, preserves_flags));
        }
    }

    unsafe {
        // Thanks to our fiddling with the stack, RSP points to `regs`,
        // which we can directly pass to the next function.
        core::arch::asm!(
            "mov rdi, [rsp]",
            "call {}",
            sym syscall_handler_inner,
        );
    }

    // Switch back to user stack
    {
        let mut kernel_rsp: *mut *mut SyscallRegisters;
        unsafe {
            core::arch::asm!("mov {}, rsp", out(reg) kernel_rsp, options(nomem, nostack, preserves_flags));
        }

        // Ignore first value (pointer to `regs`)
        kernel_rsp = unsafe { kernel_rsp.add(1) };

        // Ready previously written user RSP on the kernel stack
        let previous_rsp = unsafe { kernel_rsp.cast::<*mut u8>().read() };

        // Finally switch back to user stack
        #[allow(clippy::pointers_in_nomem_asm_block)] // False positive
        unsafe {
            core::arch::asm!("mov rsp, {}", in(reg) previous_rsp, options(nomem, nostack, preserves_flags));
        }
    }
}

#[inline(never)]
/// Performs the standardization of arguments and call to the kernel syscall handler.
///
/// Called by the above function after stack switching
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
    // it will prevent scheduling other threads
    unsafe { SFMask::write(Rflags::IF) };

    STACKS[locals!().core_id()].call_once(|| Vec::with_capacity(SYSCALL_STACK_SIZE));

    unsafe { Efer::insert_flags(Efer::SYSTEM_CALL_EXTENSIONS) };
}
