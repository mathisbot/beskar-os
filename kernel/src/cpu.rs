pub mod apic;
pub mod context;
pub mod cpuid;
pub mod gdt;
pub mod hpet;
pub mod interrupts;
pub mod rand;

pub fn init() {
    cpuid::check_cpuid();
    crate::debug!("CPU Vendor: {:?}", cpuid::get_cpu_vendor());
}
