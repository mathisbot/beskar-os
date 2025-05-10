use beskar_hal::registers::Rflags;
pub use core::arch::x86_64::CpuidResult;
use core::sync::atomic::{AtomicU32, Ordering};

const EXTENDED_MASK: u32 = 0x8000_0000;

static CPUID_MAX_LEAF: AtomicU32 = AtomicU32::new(0);
static EXTENDED_MAX_LEAF: AtomicU32 = AtomicU32::new(EXTENDED_MASK);

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Leaf(u32);

impl Leaf {
    #[must_use]
    #[inline]
    pub const fn new(leaf: u32) -> Self {
        Self(leaf)
    }

    #[must_use]
    #[inline]
    pub const fn as_u32(self) -> u32 {
        self.0
    }

    #[must_use]
    #[inline]
    pub const fn is_extended(self) -> bool {
        self.0 & EXTENDED_MASK != 0
    }
}

impl From<Leaf> for u32 {
    #[inline]
    fn from(leaf: Leaf) -> Self {
        leaf.as_u32()
    }
}

impl From<u32> for Leaf {
    #[inline]
    fn from(leaf: u32) -> Self {
        Self::new(leaf)
    }
}

#[must_use]
#[inline]
/// Stabilized version of the `__cpuid` intrinsic
///
/// # Panics
///
/// Panics if the CPUID leaf is not supported.
/// Check `get_highest_supported_leaf` and `get_highest_supported_xleaf`
/// to get the highest supported leaves.
pub fn cpuid(leaf: Leaf) -> CpuidResult {
    // No need to check for CPUID support.
    // It is the first thing getting checked in the kernel.
    assert!(
        if leaf.is_extended() {
            leaf <= get_highest_supported_xleaf()
        } else {
            leaf <= get_highest_supported_leaf()
        },
        "CPUID leaf is not supported"
    );
    unsafe { core::arch::x86_64::__cpuid(leaf.as_u32()) }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Every meaningful CPUID register
pub enum CpuidReg {
    Eax,
    Ebx,
    Ecx,
    Edx,
}

impl CpuidReg {
    #[must_use]
    pub const fn extract_from(self, cpuid_res: CpuidResult) -> u32 {
        match self {
            Self::Eax => cpuid_res.eax,
            Self::Ebx => cpuid_res.ebx,
            Self::Ecx => cpuid_res.ecx,
            Self::Edx => cpuid_res.edx,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CpuFeature {
    leaf: Leaf,
    reg: CpuidReg,
    bit: u8,
    name: &'static str,
}

impl core::fmt::Display for CpuFeature {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.name)
    }
}

// TODO: Add more features!
impl CpuFeature {
    // LEAF 1

    pub const FPU: Self = Self {
        leaf: Leaf::new(1),
        reg: CpuidReg::Edx,
        bit: 0,
        name: "FPU",
    };
    pub const PSE: Self = Self {
        leaf: Leaf::new(1),
        reg: CpuidReg::Edx,
        bit: 3,
        name: "PSE",
    };
    pub const TSC: Self = Self {
        leaf: Leaf::new(1),
        reg: CpuidReg::Edx,
        bit: 4,
        name: "TSC",
    };
    pub const MSR: Self = Self {
        leaf: Leaf::new(1),
        reg: CpuidReg::Edx,
        bit: 5,
        name: "MSR",
    };
    pub const APIC_ONBOARD: Self = Self {
        leaf: Leaf::new(1),
        reg: CpuidReg::Edx,
        bit: 9,
        name: "APIC",
    };
    pub const PAT: Self = Self {
        leaf: Leaf::new(1),
        reg: CpuidReg::Edx,
        bit: 16,
        name: "PAT",
    };
    pub const FXSR: Self = Self {
        leaf: Leaf::new(1),
        reg: CpuidReg::Edx,
        bit: 24,
        name: "FXSR",
    };
    pub const SSE: Self = Self {
        leaf: Leaf::new(1),
        reg: CpuidReg::Edx,
        bit: 25,
        name: "SSE",
    };
    pub const SSE2: Self = Self {
        leaf: Leaf::new(1),
        reg: CpuidReg::Edx,
        bit: 26,
        name: "SSE2",
    };

    pub const SSE3: Self = Self {
        leaf: Leaf::new(1),
        reg: CpuidReg::Ecx,
        bit: 0,
        name: "SSE3",
    };
    pub const PCID: Self = Self {
        leaf: Leaf::new(1),
        reg: CpuidReg::Ecx,
        bit: 17,
        name: "PCID",
    };
    pub const X2APIC: Self = Self {
        leaf: Leaf::new(1),
        reg: CpuidReg::Ecx,
        bit: 21,
        name: "X2APIC",
    };
    pub const XSAVE: Self = Self {
        leaf: Leaf::new(1),
        reg: CpuidReg::Ecx,
        bit: 26,
        name: "XSAVE",
    };
    pub const RDRAND: Self = Self {
        leaf: Leaf::new(1),
        reg: CpuidReg::Ecx,
        bit: 30,
        name: "RDRAND",
    };

    // LEAF 7

    pub const FSGSBASE: Self = Self {
        leaf: Leaf::new(7),
        reg: CpuidReg::Ebx,
        bit: 0,
        name: "FSGSBASE",
    };
    pub const INVPCID: Self = Self {
        leaf: Leaf::new(7),
        reg: CpuidReg::Ebx,
        bit: 10,
        name: "INVPCID",
    };

    // XLEAF 1

    pub const SYSCALL: Self = Self {
        leaf: Leaf::new(0x8000_0001),
        reg: CpuidReg::Edx,
        bit: 11,
        name: "SYSCALL",
    };
    pub const TCE: Self = Self {
        leaf: Leaf::new(0x8000_0001),
        reg: CpuidReg::Ecx,
        bit: 17,
        name: "TCE",
    };
}

/// List of required features for the kernel to run
///
/// Please keep the list sorted by leaf number
const REQUIRED_FEATURES: [CpuFeature; 4] = [
    // Leaf 1
    // CpuFeature::FPU,
    CpuFeature::PSE,
    CpuFeature::MSR,
    // CpuFeature::PAT,
    // CpuFeature::FXSR,
    CpuFeature::XSAVE,
    // XLeaf 1
    CpuFeature::SYSCALL,
];

/// Routine to check if the CPU supports all required features,
/// including the CPUID instruction
pub fn check_cpuid() {
    assert!(cpuid_supported(), "CPUID instruction is not supported");

    let mut current_leaf = Leaf::new(0);
    let mut cpuid_res = cpuid(current_leaf);
    CPUID_MAX_LEAF.store(cpuid_res.eax, Ordering::Release);

    current_leaf = Leaf::new(EXTENDED_MASK);
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
    let cpuid_res = cpuid(Leaf::new(0));

    let mut vendor = [0; 12];
    vendor[..4].copy_from_slice(&cpuid_res.ebx.to_ne_bytes());
    vendor[4..8].copy_from_slice(&cpuid_res.edx.to_ne_bytes());
    vendor[8..12].copy_from_slice(&cpuid_res.ecx.to_ne_bytes());

    CpuVendor::from(&vendor)
}

#[must_use]
/// Check if a CPU feature is supported
pub fn check_feature(feature: CpuFeature) -> bool {
    let leaf = feature.leaf;
    let extended = leaf.is_extended();

    // Make sure CPUID won't panic
    if (!extended && leaf > get_highest_supported_leaf())
        || (extended && leaf > get_highest_supported_xleaf())
    {
        return false;
    }

    let cpuid_res = cpuid(leaf);
    let reg = feature.reg.extract_from(cpuid_res);

    (reg >> feature.bit) & 1 == 1
}

#[must_use]
#[inline]
/// Get the highest supported leaf
pub fn get_highest_supported_leaf() -> Leaf {
    Leaf::new(CPUID_MAX_LEAF.load(Ordering::Acquire))
}

#[must_use]
#[inline]
/// Get the highest supported extended leaf
/// (EAX >= `0x8000_0000`)
pub fn get_highest_supported_xleaf() -> Leaf {
    Leaf::new(EXTENDED_MAX_LEAF.load(Ordering::Acquire))
}
