use core::ops::RangeInclusive;

use alloc::vec::Vec;
use beskar_core::arch::commons::PhysAddr;

use super::{Sdt, SdtHeader};

crate::impl_sdt!(Mcfg);

#[derive(Debug, Clone)]
pub struct ParsedMcfg {
    configuration_spaces: Vec<ParsedConfigurationSpace>,
}

#[derive(Debug, Clone, Copy)]
pub struct ParsedConfigurationSpace {
    /// Base address of the enhanced configuration mechanism
    offset: u64,
    /// PCI Segment Group Number
    segment_group_number: u16,
    /// Start PCI Bus Number
    start_pci_bus_number: u8,
    /// End PCI Bus Number
    end_pci_bus_number: u8,
}

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
struct ConfigurationSpace {
    offset: u64,
    segment_group_number: u16,
    start_pci_bus_number: u8,
    end_pci_bus_number: u8,
    _reserved: u32,
}

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
struct McfgHeader {
    sdt_header: SdtHeader,
    _reserved: u64,
}

// See <https://uefi.org/htmlspecs/ACPI_Spec_6_4_html/05_ACPI_Software_Programming_Model/ACPI_Software_Programming_Model.html#multiple-apic-description-table-madt>
impl Mcfg {
    #[must_use]
    pub fn parse(&self) -> ParsedMcfg {
        let nb_cs = (usize::try_from(self.length()).unwrap() - size_of::<McfgHeader>())
            / size_of::<ConfigurationSpace>();

        let mut configuration_spaces = Vec::with_capacity(nb_cs);

        let base = unsafe {
            self.start_vaddr
                .as_ptr::<ConfigurationSpace>()
                .byte_add(core::mem::size_of::<McfgHeader>())
        };
        for i in 0..nb_cs {
            let cs = unsafe { base.add(i).read() };
            // Unpack the configuration space
            let pcs = ParsedConfigurationSpace {
                offset: cs.offset,
                segment_group_number: cs.segment_group_number,
                start_pci_bus_number: cs.start_pci_bus_number,
                end_pci_bus_number: cs.end_pci_bus_number,
            };
            configuration_spaces.push(pcs);
        }

        ParsedMcfg {
            configuration_spaces,
        }
    }
}

impl ParsedMcfg {
    #[must_use]
    #[inline]
    pub fn configuration_spaces(&self) -> &[ParsedConfigurationSpace] {
        &self.configuration_spaces
    }
}

impl ParsedConfigurationSpace {
    #[must_use]
    #[inline]
    pub const fn offset(&self) -> u64 {
        self.offset
    }

    #[must_use]
    #[inline]
    pub const fn segment_group_number(&self) -> u16 {
        self.segment_group_number
    }

    #[must_use]
    #[inline]
    pub const fn start_pci_bus_number(&self) -> u8 {
        self.start_pci_bus_number
    }

    #[must_use]
    #[inline]
    pub const fn end_pci_bus_number(&self) -> u8 {
        self.end_pci_bus_number
    }

    #[must_use]
    pub fn address_range(&self) -> RangeInclusive<PhysAddr> {
        // Minimum bus, device, function, and register numbers
        let start_paddr =
            PhysAddr::new(self.offset + (u64::from(self.start_pci_bus_number()) << 20));
        // Maximum bus, device, function, and register numbers
        let end_paddr = PhysAddr::new(
            self.offset
                + (u64::from(self.end_pci_bus_number()) << 20)
                + (31 << 15)
                + (7 << 12)
                + 0xFF,
        );

        start_paddr..=end_paddr
    }
}
