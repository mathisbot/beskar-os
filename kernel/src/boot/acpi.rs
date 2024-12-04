#![allow(dead_code, unused_variables)] // TODO: Remove

use core::sync::atomic::AtomicU8;

use spin::Once;
use x86_64::PhysAddr;

mod rsdp;
use rsdp::Rsdp;
pub mod sdt;
use sdt::Rsdt;

static ACPI_REVISION: AcpiRevisionStorage = AcpiRevisionStorage::uninit();

pub static ACPI: Once<Acpi> = Once::new();

pub fn init(rsdp_paddr: PhysAddr) {
    ACPI.call_once(|| Acpi::from_rsdp_paddr(rsdp_paddr));
}

/// Advanced Configuration and Power Interface (ACPI) support.
pub struct Acpi {
    revision: AcpiRevision,

    // Related to MADT
    lapic_paddr: PhysAddr,
    io_apic_id: Option<u8>,
    io_apic_addr: Option<u32>,
    gsi_base: Option<u32>,
}

impl Acpi {
    #[must_use]
    pub fn from_rsdp_paddr(rsdp_paddr: PhysAddr) -> Self {
        let cpuid_res = unsafe { core::arch::x86_64::__cpuid(1) };
        assert_eq!((cpuid_res.edx >> 22) & 1, 1, "CPU does not support ACPI");

        let rsdt_paddr = Rsdp::load(rsdp_paddr).rsdt_paddr();
        let rsdt = Rsdt::load(rsdt_paddr);

        let madt_paddr = rsdt
            .locate_table(sdt::Signature::Madt)
            .expect("MADT not found");
        let fadt_paddr = rsdt
            .locate_table(sdt::Signature::Fadt)
            .expect("FADT not found");
        let hpet_paddr = rsdt.locate_table(sdt::Signature::Hpet);
        if hpet_paddr.is_some() {
            log::debug!("HPET found");
        }

        drop(rsdt);

        let madt = sdt::madt::Madt::load(madt_paddr).parse();
        let fadt = sdt::fadt::Fadt::load(fadt_paddr).parse();
        let hpet = hpet_paddr.map(|paddr| sdt::hpet_table::HpetTable::load(paddr).parse());
        if let Some(hpet_info) = hpet {
            crate::cpu::hpet::init(hpet_info);
        }

        Self {
            revision: ACPI_REVISION.load(),

            lapic_paddr: madt.lapic_paddr(),
            io_apic_id: madt.io_apic_id(),
            io_apic_addr: madt.io_apic_addr(),
            gsi_base: madt.gsi_base(),
        }
    }

    #[must_use]
    #[inline]
    pub const fn lapic_paddr(&self) -> PhysAddr {
        self.lapic_paddr
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum AcpiRevision {
    V1 = 1,
    V2 = 2,
}

impl From<AcpiRevision> for u8 {
    fn from(revision: AcpiRevision) -> Self {
        revision as Self
    }
}

impl From<u8> for AcpiRevision {
    fn from(revision: u8) -> Self {
        match revision {
            1 => Self::V1,
            2 => Self::V2,
            x => unreachable!("Unknown ACPI revision: {}", x),
        }
    }
}

pub struct AcpiRevisionStorage(AtomicU8);

impl AcpiRevisionStorage {
    pub const fn uninit() -> Self {
        Self(AtomicU8::new(0))
    }

    pub fn store(&self, revision: AcpiRevision) {
        self.0
            .store(u8::from(revision), core::sync::atomic::Ordering::Relaxed);
    }

    pub fn load(&self) -> AcpiRevision {
        AcpiRevision::from(self.0.load(core::sync::atomic::Ordering::Relaxed))
    }
}
