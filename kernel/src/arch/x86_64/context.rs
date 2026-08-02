use beskar_hal::registers::{Cr0, Rflags};

#[unsafe(naked)]
/// Switches the current stack and CR3 to the ones provided.
///
/// # Safety
///
/// Interrupts must be disabled when calling this function.
/// This function will re-enable interrupts before returning.
pub unsafe extern "C" fn switch(old_stack: *mut *mut u8, new_stack: *const u8, cr3: u64) {
    // Thanks to the C calling convention,
    // the arguments are in the correct registers:
    // RDI = old_stack
    // RSI = new_stack
    // RDX = cr3
    core::arch::naked_asm!(
        // Push the current context to the stack
        "pushfq",
        "push rbx",
        "push rbp",
        "push rdi", // TODO: Don't save RDI
        "push r12",
        "push r13",
        "push r14",
        "push r15",
        // Update stack pointer
        "mov [rdi], rsp",
        "mov rsp, rsi",
        // Set TS bit in CR0
        "mov rax, cr0",
        "or rax, {ts}",
        "mov cr0, rax",
        // Check if CR3 is different
        "mov rax, cr3",
        "cmp rax, rdx",
        "je 2f",
        // Load the new CR3 ONLY if it is different
        "mov cr3, rdx",
        "2:",
        // Load the new context from the stack
        "pop r15",
        "pop r14",
        "pop r13",
        "pop r12",
        "pop rdi",
        "pop rbp",
        "pop rbx",
        "popfq",
        // Finally, return to the new stack
        "sti",
        "ret",
        ts = const Cr0::TASK_SWITCHED,
    );
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
/// Registers that are relevant for the thread context.
pub struct ThreadRegisters {
    r15: u64,
    r14: u64,
    r13: u64,
    r12: u64,
    rdi: u64,
    rbp: u64,
    rbx: u64,
    rflags: u64,
    rip: u64,
}

impl ThreadRegisters {
    #[must_use]
    #[inline]
    pub fn new(entry: extern "C" fn(usize) -> !, arg0: usize) -> Self {
        let rip = u64::try_from(entry as usize).unwrap();
        let rdi = u64::try_from(arg0).unwrap();
        let rflags = Rflags::IF;
        Self {
            r15: 0,
            r14: 0,
            r13: 0,
            r12: 0,
            rdi,
            rbp: 0,
            rbx: 0,
            rflags,
            rip,
        }
    }

    #[must_use]
    #[inline]
    pub const fn as_raw(&self) -> &[u8; size_of::<Self>()] {
        let ptr = core::ptr::from_ref(self).cast();
        unsafe { &*ptr }
    }
}
