use beskar_core::{
    arch::{
        commons::paging::{CacheFlush, Flags, FrameAllocator, M4KiB, Mapper},
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
static SYSCALL_STACK_PTRS: [Once<*mut u8>; 256] = [const { Once::uninit() }; 256];

const SYSCALL_STACK_NB_PAGES: u64 = 4; // 16 KiB

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

    let kernel_stack = *SYSCALL_STACK_PTRS[locals!().core_id()].get().unwrap();
    unsafe {
        core::arch::asm!(
            "mov {ustack}, rsp", // Keep track of user stack (0)
            "mov rsp, {}", // Switch to kernel stack
            "push {ustack}", // Keep track of user stack (1)
            "call {}", // Perform the function call with `regs` in rdi
            "pop rsp", // Switch back to user stack
            in(reg) kernel_stack,
            sym syscall_handler_inner,
            in("rdi") regs,
            ustack = out(reg) _,
        );
    }
}

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

    SYSCALL_STACK_PTRS[locals!().core_id()].call_once(|| allocate_stack(SYSCALL_STACK_NB_PAGES));

    unsafe { Efer::insert_flags(Efer::SYSTEM_CALL_EXTENSIONS) };
}

// Allocate a stack for the syscall handler and return the top of the stack
fn allocate_stack(nb_pages: u64) -> *mut u8 {
    let (page_range, _guard_page) = crate::mem::page_alloc::with_page_allocator(|palloc| {
        palloc.allocate_guarded::<M4KiB>(nb_pages).unwrap()
    });

    crate::mem::frame_alloc::with_frame_allocator(|fralloc| {
        crate::mem::address_space::with_kernel_pt(|kpt| {
            for page in page_range.clone() {
                let frame = fralloc.allocate_frame().unwrap();
                kpt.map(page, frame, Flags::PRESENT | Flags::WRITABLE, fralloc)
                    .flush();
            }
        })
    });

    // We need the stack to be 16-byte aligned.
    // Here, it is 4096-byte aligned!
    let stack_bottom = page_range.start.start_address() + page_range.size();
    debug_assert_eq!(stack_bottom.align_down(16), stack_bottom);
    stack_bottom.as_mut_ptr()
}
