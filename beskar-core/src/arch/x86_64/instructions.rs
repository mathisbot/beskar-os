#[inline]
pub unsafe fn load_tss(selector: u16) {
    unsafe {
        core::arch::asm!("ltr {:x}", in(reg) selector, options(nostack, preserves_flags));
    }
}

#[inline]
pub unsafe fn load_idt(descriptor: &super::structures::DescriptorTable) {
    unsafe {
        core::arch::asm!(
            "lidt [{}]",
            in(reg) descriptor,
            options(nostack, readonly, preserves_flags)
        );
    }
}

#[inline]
pub unsafe fn load_gdt(descriptor: &super::structures::DescriptorTable) {
    unsafe {
        core::arch::asm!(
            "lgdt [{}]",
            in(reg) descriptor,
            options(nostack, readonly, preserves_flags)
        );
    }
}

/// This value can be used to fill the stack when debugging stack overflows.
pub const STACK_DEBUG_INSTR: u8 = 0xCC;
