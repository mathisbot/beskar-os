//! DSDT (Differentiated System Description Table)
//!
//! This module heaviliy relies on AML parsing and validation.
//! See <https://uefi.org/htmlspecs/ACPI_Spec_6_4_html/05_ACPI_Software_Programming_Model/ACPI_Software_Programming_Model.html#aml-encoding>
//! for more information.

use super::super::aml::Aml;
use super::{Sdt, SdtHeader};

super::impl_sdt!(Dsdt);

#[derive(Debug, Copy, Clone)]
#[repr(C, packed)]
struct RawDsdt {
    header: SdtHeader,
    /// Bytes of AML code.
    def_block: [u8; 0],
}

impl RawDsdt {
    #[must_use]
    #[inline]
    pub fn aml_bytes(&self) -> &[u8] {
        let data = self.def_block.as_ptr();
        let len = usize::try_from(self.header.length).unwrap() - size_of::<SdtHeader>();
        unsafe { core::slice::from_raw_parts(data, len) }
    }
}

impl<M: driver_api::PhysicalMapper<beskar_core::arch::paging::M4KiB>> Dsdt<M> {
    #[must_use]
    pub fn parse(&self) -> ParsedDsdt {
        assert_eq!(
            self.signature(),
            super::Signature::Dsdt.as_bytes(),
            "Invalid DSDT signature"
        );

        let raw = {
            let raw_ptr = self.start_vaddr.as_ptr::<RawDsdt>();
            unsafe { &*raw_ptr }
        };

        let aml = Aml::parse(raw.aml_bytes());

        ParsedDsdt { aml }
    }
}

pub struct ParsedDsdt {
    aml: Option<Aml>,
}

impl ParsedDsdt {
    #[must_use]
    #[inline]
    pub const fn aml(&self) -> Option<&Aml> {
        self.aml.as_ref()
    }
}
