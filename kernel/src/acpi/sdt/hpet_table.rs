use crate::impl_sdt;

use super::{GenericAddress, RawGenericAddress, Sdt, SdtHeader};

impl_sdt!(HpetTable);

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
/// Refer to page 30 of
/// <https://www.intel.com/content/dam/www/public/us/en/documents/technical-specifications/software-developers-hpet-spec-1-0a.pdf>
struct FullHpet {
    header: SdtHeader,
    pci_vendor_id: u16,
    /// Bit 15: Legacy Replacement IRQ Routing Capable
    /// Bit 14: RESERVED
    /// Bit 13: Count Size Capable
    /// Bits 8-12: Number of comparators
    /// Bits 0-7: Hardware revision ID
    etb_id: u16,
    base_address: RawGenericAddress,
    hpet_number: u8,
    minimum_tick: u16,
    /// Bits 0-3: Page Protection
    /// Bits 4-7: Oem attributes (IGNORE)
    page_prot_oem: u8,
}

impl FullHpet {
    #[must_use]
    #[inline]
    pub const fn irq_routing_capable(&self) -> bool {
        (self.etb_id >> 15) & 1 == 1
    }

    #[must_use]
    #[inline]
    pub const fn count_size_capable(&self) -> bool {
        (self.etb_id >> 13) & 1 == 1
    }

    #[must_use]
    #[inline]
    pub fn comparator_count(&self) -> u8 {
        u8::try_from((self.etb_id >> 8) & 0b0001_1111).unwrap()
    }

    #[must_use]
    #[inline]
    pub fn hardware_rev_id(&self) -> u8 {
        u8::try_from(self.etb_id & 0b0111_1111).unwrap()
    }

    #[must_use]
    #[inline]
    pub fn page_protection(&self) -> PageProtection {
        (self.page_prot_oem & 0b1111).into()
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PageProtection {
    NoGuarantees = 0,
    FourKb = 1,
    SixtyFourKb = 2,
    Reserved,
}

impl From<u8> for PageProtection {
    fn from(value: u8) -> Self {
        assert!(value <= 0b1111);
        match value {
            0 => Self::NoGuarantees,
            1 => Self::FourKb,
            2 => Self::SixtyFourKb,
            _ => Self::Reserved,
        }
    }
}

impl HpetTable {
    pub fn parse(&self) -> ParsedHpetTable {
        assert_eq!(
            usize::try_from(self.length()).unwrap(),
            size_of::<FullHpet>(),
            "HPET size mismatch"
        );

        let hpet = unsafe { self.start_vaddr.as_ptr::<FullHpet>().read_unaligned() };
        assert_eq!(self.revision(), 1, "HPET revision must be 1");

        ParsedHpetTable {
            base_address: hpet.base_address.into(),
            // frequency,
            minimal_tick: hpet.minimum_tick,
        }
    }
}

pub struct ParsedHpetTable {
    base_address: GenericAddress,
    // frequency: u64,
    minimal_tick: u16,
}

impl ParsedHpetTable {
    #[must_use]
    #[inline]
    pub const fn base_address(&self) -> GenericAddress {
        self.base_address
    }

    #[must_use]
    #[inline]
    pub const fn minimal_tick(&self) -> u16 {
        self.minimal_tick
    }
}
