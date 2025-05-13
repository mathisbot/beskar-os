use beskar_core::arch::PhysAddr;
use core::sync::atomic::AtomicU8;
use hyperdrive::once::Once;

mod aml;
mod rsdp;
pub mod sdt;
use sdt::{
    Rsdt,
    dsdt::ParsedDsdt,
    fadt::ParsedFadt,
    hpet_table::ParsedHpetTable,
    madt::ParsedMadt,
    mcfg::{self, ParsedMcfg},
};

static ACPI_REVISION: AcpiRevisionStorage = AcpiRevisionStorage::uninit();

pub static ACPI: Once<Acpi> = Once::uninit();

pub fn init(rsdp_paddr: PhysAddr) {
    let acpi = Acpi::from_rsdp_paddr(rsdp_paddr);
    ACPI.call_once(|| acpi);
    video::debug!("ACPI initialized");
}

/// Advanced Configuration and Power Interface (ACPI) support.
pub struct Acpi {
    madt: ParsedMadt,
    fadt: ParsedFadt,
    hpet: Option<ParsedHpetTable>,
    mcfg: Option<ParsedMcfg>,
    dsdt: ParsedDsdt,
}

impl Acpi {
    #[must_use]
    pub fn from_rsdp_paddr(rsdp_paddr: PhysAddr) -> Self {
        let rsdt_paddr = rsdp::Rsdp::load(rsdp_paddr).rsdt_paddr();
        let rsdt = Rsdt::load(rsdt_paddr);

        let madt_paddr = rsdt
            .locate_table(sdt::Signature::Madt)
            .expect("MADT not found");
        let fadt_paddr = rsdt
            .locate_table(sdt::Signature::Fadt)
            .expect("FADT not found");
        // TODO: Support multiple HPET blocks?
        let hpet_paddr = rsdt.locate_table(sdt::Signature::Hpet);
        if hpet_paddr.is_none() {
            video::warn!("HPET table not found");
        }
        let mcfg_paddr = rsdt.locate_table(sdt::Signature::Mcfg);
        if mcfg_paddr.is_none() {
            video::warn!("MCFG table not found");
        }

        drop(rsdt);

        let madt = sdt::madt::Madt::load(madt_paddr).parse();
        let fadt = sdt::fadt::Fadt::load(fadt_paddr).parse();
        let hpet = hpet_paddr.map(|paddr| sdt::hpet_table::HpetTable::load(paddr).parse());
        let mcfg = mcfg_paddr.map(|paddr| mcfg::Mcfg::load(paddr).parse());

        let dsdt = sdt::dsdt::Dsdt::load(fadt.dsdt()).parse();

        Self {
            madt,
            fadt,
            hpet,
            mcfg,
            dsdt,
        }
    }

    #[must_use]
    #[inline]
    pub const fn madt(&self) -> &ParsedMadt {
        &self.madt
    }

    #[must_use]
    #[inline]
    pub const fn fadt(&self) -> &ParsedFadt {
        &self.fadt
    }

    #[must_use]
    #[inline]
    pub const fn hpet(&self) -> Option<&ParsedHpetTable> {
        self.hpet.as_ref()
    }

    #[must_use]
    #[inline]
    pub const fn mcfg(&self) -> Option<&ParsedMcfg> {
        self.mcfg.as_ref()
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

impl TryFrom<u8> for AcpiRevision {
    type Error = ();

    fn try_from(revision: u8) -> Result<Self, Self::Error> {
        match revision {
            1 => Ok(Self::V1),
            2 => Ok(Self::V2),
            _ => Err(()),
        }
    }
}

struct AcpiRevisionStorage(AtomicU8);

impl AcpiRevisionStorage {
    #[must_use]
    #[inline]
    pub const fn uninit() -> Self {
        Self(AtomicU8::new(0))
    }

    pub fn store(&self, revision: AcpiRevision) {
        self.0
            .store(u8::from(revision), core::sync::atomic::Ordering::Relaxed);
    }

    #[must_use]
    pub fn load(&self) -> AcpiRevision {
        AcpiRevision::try_from(self.0.load(core::sync::atomic::Ordering::Relaxed)).unwrap()
    }
}
