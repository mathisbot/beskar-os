pub mod apic;
pub mod cpuid;
pub mod gdt;
pub mod hpet;
pub mod interrupts;

pub fn init() {
    cpuid::check_cpuid();
    crate::serdebug!("CPU Vendor: {:?}", cpuid::get_cpu_vendor());
}
