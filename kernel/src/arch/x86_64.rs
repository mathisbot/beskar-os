use beskar_hal::registers::XCr0;

pub const XSAVE_AREA_MAX_SIZE: usize = size_of::<beskar_hal::structures::SseSave>();

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
    crate::debug!("CPU Vendor: {:?}", cpuid::get_cpu_vendor());

    prepare_sse();
    prepare_fsgsbase();
}

fn prepare_sse() {
    use beskar_hal::registers::{Cr0, Cr4};

    assert!(
        cpuid::check_feature(cpuid::CpuFeature::XSAVE),
        "CPU does not support XSAVE"
    );

    // Prepare CR0
    let mut cr0 = Cr0::read();
    cr0 |= Cr0::MONITOR_COPROCESSOR;
    cr0 &= !Cr0::EMULATE_COPROCESSOR;
    unsafe { Cr0::write(cr0) };

    // Prepare CR4
    unsafe { Cr4::add(Cr4::OSFXSR | Cr4::OSXMMEXCPT | Cr4::OSXSAVE) };
    debug_assert!(cpuid::check_feature(cpuid::CpuFeature::OSXSAVE));

    let res = cpuid::cpuid_count(cpuid::Leaf::new(0xD), 0);
    let bitmap = u64::from(res.edx) << 32 | u64::from(res.eax);
    let xcr0 = XCr0::X87 | XCr0::SSE | XCr0::AVX;
    assert_eq!(
        xcr0 & !bitmap,
        0,
        "CPU does not support required XSAVE features"
    );

    // Prepare XCR0
    unsafe { XCr0::write(xcr0) };

    let res = cpuid::cpuid_count(cpuid::Leaf::new(0xD), 0);
    let max_curr_sz = res.ebx;
    assert!(usize::try_from(max_curr_sz).unwrap() <= XSAVE_AREA_MAX_SIZE);

    let res = cpuid::cpuid_count(cpuid::Leaf::new(0xD), 1);
    let xcr0_features = res.eax;
    assert_eq!(xcr0_features & 0b10, 0b10, "CPU does not support XSAVEC");
}

fn prepare_fsgsbase() {
    use beskar_hal::registers::Cr4;

    // All modern CPUs should support it
    assert!(
        cpuid::check_feature(cpuid::CpuFeature::FSGSBASE),
        "CPU does not support FSGSBASE"
    );
    unsafe { Cr4::add(Cr4::FSGSBASE) };
}

#[inline]
pub fn halt() {
    beskar_hal::instructions::halt();
}
