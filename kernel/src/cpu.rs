use x86_64::registers::rflags;

pub mod apic;
pub mod gdt;
pub mod hpet;
pub mod interrupts;

pub fn check_cpuid() {
    let mut rflags = rflags::read();
    let old_id_flag = rflags.intersection(rflags::RFlags::ID);
    rflags.toggle(rflags::RFlags::ID);

    // Depending on the CPU, this line can cause an invalid opcode exception, crashing the whole system.
    //
    // This is not a real problem, as CPU that don't support CPUID don't support APIC either,
    // so the kernel can't run on them anyway.
    unsafe { rflags::write(rflags) };

    // Check that ID flag was toggled
    let new_id_flag = rflags::read().intersection(rflags::RFlags::ID);
    assert_ne!(
        old_id_flag, new_id_flag,
        "CPUID instruction is not supported"
    );

    // CPUID instruction IS supported

    // Assert CPU supports at least SSE2
    let cpuid = unsafe { core::arch::x86_64::__cpuid(1) };
    assert_eq!((cpuid.edx >> 26) & 1, 1, "CPU does not support SSE2");
}
