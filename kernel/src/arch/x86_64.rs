pub mod ap;
pub mod apic;
pub mod context;
pub mod cpuid;
pub mod fpu;
pub mod gdt;
pub mod interrupts;
pub mod locals;
pub mod rand;
pub mod syscall;
pub mod userspace;

pub fn init() {
    cpuid::check_cpuid();
    video::debug!("CPU Vendor: {:?}", cpuid::get_cpu_vendor());

    prepare_sse();
}

fn prepare_sse() {
    use beskar_hal::registers::{Cr0, Cr4};

    // Prepare CR0
    let mut cr0 = Cr0::read();
    cr0 |= Cr0::MONITOR_COPROCESSOR;
    cr0 &= !Cr0::EMULATE_COPROCESSOR;
    unsafe { Cr0::write(cr0) };

    // Prepare CR4
    let mut cr4 = Cr4::read();
    cr4 |= Cr4::OSFXSR;
    cr4 |= Cr4::OSXMMEXCPT;
    unsafe { Cr4::write(cr4) };

    unsafe { beskar_hal::instructions::fpu_init() };
}

#[inline]
pub fn halt() {
    beskar_hal::instructions::halt();
}
