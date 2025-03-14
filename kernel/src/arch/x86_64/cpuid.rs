use beskar_core::arch::x86_64::registers::Rflags;
pub use core::arch::x86_64::CpuidResult;
use core::sync::atomic::{AtomicU32, Ordering};

const EXTENDED_MASK: u32 = 0x8000_0000;

static CPUID_MAX_LEAF: AtomicU32 = AtomicU32::new(0);
static EXTENDED_MAX_LEAF: AtomicU32 = AtomicU32::new(EXTENDED_MASK);

#[must_use]
#[inline]
/// Stabilized version of the `__cpuid` intrinsic
///
/// # Panics
///
/// Panics if the CPUID leaf is not supported.
/// Check `get_highest_supported_leaf` and `get_highest_supported_xleaf`
/// to get the highest supported leaves.
pub fn cpuid(leaf: u32) -> CpuidResult {
    let extended = leaf & EXTENDED_MASK != 0;

    // No need to check for CPUID support.
    // It is the first thing getting checked in the kernel.
    assert!(
        if extended {
            leaf <= EXTENDED_MAX_LEAF.load(Ordering::Acquire)
        } else {
            leaf <= CPUID_MAX_LEAF.load(Ordering::Acquire)
        },
        "CPUID leaf is not supported"
    );
    unsafe { core::arch::x86_64::__cpuid(leaf) }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Every meaningful CPUID register
pub enum CpuidReg {
    // Eax,
    Ebx,
    Ecx,
    Edx,
}

impl CpuidReg {
    #[must_use]
    pub const fn extract_from(self, cpuid_res: CpuidResult) -> u32 {
        match self {
            // Self::Eax => cpuid_res.eax,
            Self::Ebx => cpuid_res.ebx,
            Self::Ecx => cpuid_res.ecx,
            Self::Edx => cpuid_res.edx,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CpuFeature {
    leaf: u32,
    reg: CpuidReg,
    bit: u32,
    name: &'static str,
}

impl core::fmt::Display for CpuFeature {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.name)
    }
}

// TODO: Add more features!
#[allow(dead_code)]
impl CpuFeature {
    // LEAF 1

    pub const FPU: Self = Self {
        leaf: 1,
        reg: CpuidReg::Edx,
        bit: 0,
        name: "FPU",
    };
    pub const PSE: Self = Self {
        leaf: 1,
        reg: CpuidReg::Edx,
        bit: 3,
        name: "PSE",
    };
    pub const TSC: Self = Self {
        leaf: 1,
        reg: CpuidReg::Edx,
        bit: 4,
        name: "TSC",
    };
    pub const MSR: Self = Self {
        leaf: 1,
        reg: CpuidReg::Edx,
        bit: 5,
        name: "MSR",
    };
    pub const APIC_ONBOARD: Self = Self {
        leaf: 1,
        reg: CpuidReg::Edx,
        bit: 9,
        name: "APIC",
    };
    pub const PAT: Self = Self {
        leaf: 1,
        reg: CpuidReg::Edx,
        bit: 16,
        name: "PAT",
    };
    pub const FXSR: Self = Self {
        leaf: 1,
        reg: CpuidReg::Edx,
        bit: 24,
        name: "FXSR",
    };
    pub const SSE: Self = Self {
        leaf: 1,
        reg: CpuidReg::Edx,
        bit: 25,
        name: "SSE",
    };
    pub const SSE2: Self = Self {
        leaf: 1,
        reg: CpuidReg::Edx,
        bit: 26,
        name: "SSE2",
    };

    pub const SSE3: Self = Self {
        leaf: 1,
        reg: CpuidReg::Ecx,
        bit: 0,
        name: "SSE3",
    };
    pub const X2APIC: Self = Self {
        leaf: 1,
        reg: CpuidReg::Ecx,
        bit: 21,
        name: "X2APIC",
    };
    pub const XSAVE: Self = Self {
        leaf: 1,
        reg: CpuidReg::Ecx,
        bit: 26,
        name: "XSAVE",
    };
    pub const RDRAND: Self = Self {
        leaf: 1,
        reg: CpuidReg::Ecx,
        bit: 30,
        name: "RDRAND",
    };

    // LEAF 7

    pub const FSGSBASE: Self = Self {
        leaf: 7,
        reg: CpuidReg::Ebx,
        bit: 0,
        name: "FSGSBASE",
    };

    // XLEAF 1

    pub const SYSCALL: Self = Self {
        leaf: 0x8000_0001,
        reg: CpuidReg::Edx,
        bit: 11,
        name: "SYSCALL",
    };
}

/// List of required features for the kernel to run
///
/// Please keep the list sorted by leaf number
const REQUIRED_FEATURES: [CpuFeature; 5] = [
    // Leaf 1
    // CpuFeature::FPU,
    CpuFeature::PSE,
    CpuFeature::MSR,
    // CpuFeature::PAT,
    // CpuFeature::FXSR,
    CpuFeature::XSAVE,
    // Leaf 7
    CpuFeature::FSGSBASE, // TLS support
    // XLeaf 1
    CpuFeature::SYSCALL,
];

/// Routine to check if the CPU supports all required features,
/// including the CPUID instruction
pub fn check_cpuid() {
    assert!(cpuid_supported(), "CPUID instruction is not supported");

    let mut current_leaf = 0;
    let mut cpuid_res = cpuid(current_leaf);
    CPUID_MAX_LEAF.store(cpuid_res.eax, Ordering::Release);

    current_leaf = EXTENDED_MASK;
    cpuid_res = cpuid(current_leaf);
    EXTENDED_MAX_LEAF.store(cpuid_res.eax, Ordering::Release);

    for feature in REQUIRED_FEATURES {
        // Avoid calling CPUID multiple times for the same leaf
        if current_leaf != feature.leaf {
            cpuid_res = cpuid(feature.leaf);
            current_leaf = feature.leaf;
        }
        let reg = feature.reg.extract_from(cpuid_res);

        assert_eq!(
            (reg >> feature.bit) & 1,
            1,
            "CPU does not support required feature: {feature}",
        );
    }
}

#[must_use]
/// Check if the CPU supports the CPUID instruction
fn cpuid_supported() -> bool {
    let mut rflags = Rflags::read();
    let old_id_flag = rflags & Rflags::ID;

    if old_id_flag != 0 {
        rflags &= !Rflags::ID;
    } else {
        rflags |= Rflags::ID;
    }

    // Depending on the CPU, this line can cause an invalid opcode exception, crashing the whole system.
    //
    // This is not a real problem, as CPUs that don't support CPUID don't support much required features,
    // so the kernel can't run on them anyway.
    unsafe { Rflags::write(rflags) };

    let new_id_flag = Rflags::read() & Rflags::ID;

    new_id_flag != old_id_flag
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpuVendor {
    Intel,
    Amd,
    Other,
}

impl From<&[u8; 12]> for CpuVendor {
    fn from(vendor: &[u8; 12]) -> Self {
        match vendor {
            b"GenuineIntel" | b"GenuineIotel" => Self::Intel,
            b"AuthenticAMD" => Self::Amd,
            _ => Self::Other,
        }
    }
}

#[must_use]
pub fn get_cpu_vendor() -> CpuVendor {
    let cpuid_res = cpuid(0);

    let mut vendor = [0; 12];
    vendor[..4].copy_from_slice(&cpuid_res.ebx.to_ne_bytes());
    vendor[4..8].copy_from_slice(&cpuid_res.edx.to_ne_bytes());
    vendor[8..12].copy_from_slice(&cpuid_res.ecx.to_ne_bytes());

    CpuVendor::from(&vendor)
}

#[must_use]
/// Check if a CPU feature is supported
pub fn check_feature(feature: CpuFeature) -> bool {
    if feature.leaf > CPUID_MAX_LEAF.load(Ordering::Acquire) {
        return false;
    }

    let cpuid_res = cpuid(feature.leaf);

    let reg = feature.reg.extract_from(cpuid_res);

    (reg >> feature.bit) & 1 == 1
}

#[must_use]
#[inline]
/// Get the highest supported leaf
pub fn get_highest_supported_leaf() -> u32 {
    CPUID_MAX_LEAF.load(Ordering::Acquire)
}

#[must_use]
#[inline]
/// Get the highest supported extended leaf
/// (EAX >= `0x8000_0000`)
pub fn get_highest_supported_xleaf() -> u32 {
    EXTENDED_MAX_LEAF.load(Ordering::Acquire)
}
