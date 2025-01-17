pub mod ap;
pub mod apic;
pub mod context;
pub mod cpuid;
pub mod gdt;
pub mod interrupts;
pub mod locals;
pub mod paging;
pub mod rand;
pub mod registers;
pub mod serial;
pub mod syscall;

pub fn init() {
    cpuid::check_cpuid();
    crate::debug!("CPU Vendor: {:?}", cpuid::get_cpu_vendor());
}

pub fn halt() {
    unsafe {
        core::arch::asm!("hlt", options(nomem, nostack, preserves_flags));
    }
}
