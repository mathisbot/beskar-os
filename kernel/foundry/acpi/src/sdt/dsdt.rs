//! DSDT (Differentiated System Description Table)
//!
//! This module heaviliy relies on AML parsing and validation.
//! See <https://uefi.org/htmlspecs/ACPI_Spec_6_4_html/05_ACPI_Software_Programming_Model/ACPI_Software_Programming_Model.html#aml-encoding>
//! for more information.
#![allow(dead_code, reason = "WIP")]

use super::super::aml::parse_aml;
use super::{Sdt, SdtHeader};

super::impl_sdt!(Dsdt);

#[derive(Debug, Copy, Clone)]
struct DefinitionBlock;

#[derive(Debug, Copy, Clone)]
#[repr(C, packed)]
struct RawDsdt {
    header: SdtHeader,
    /// Bytes of AML code.
    def_block: DefinitionBlock,
}

impl<M: ::driver_api::PhysicalMapper<::beskar_core::arch::paging::M4KiB>> Dsdt<M> {
    #[must_use]
    pub fn parse(&self) -> ParsedDsdt {
        assert_eq!(
            self.signature(),
            super::Signature::Dsdt.as_bytes(),
            "Invalid DSDT signature"
        );

        let aml_slice = {
            let aml_start = self.start_vaddr + u64::try_from(size_of::<SdtHeader>()).unwrap();
            let aml_bytes = usize::try_from(self.length()).unwrap() - size_of::<SdtHeader>();
            // Safety: Assuming data coming from DSDT is valid, the pointer is valid and the length is correct
            // (as in: there are `len * sizeof::<u8>()` bytes valid for read).
            unsafe { core::slice::from_raw_parts(aml_start.as_ptr::<u8>(), aml_bytes) }
        };

        let _res = parse_aml(aml_slice);

        ParsedDsdt {}
    }
}

pub struct ParsedDsdt {}
