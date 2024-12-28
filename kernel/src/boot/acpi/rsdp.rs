use core::mem::offset_of;

use super::AcpiRevision;
use crate::mem::page_alloc::pmap::PhysicalMapping;
use x86_64::{structures::paging::PageTableFlags, PhysAddr, VirtAddr};

#[derive(Clone, Copy, Debug)]
#[repr(C, packed)]
/// Represents the ACPI 1.0 RSDP.
struct Rsdp1 {
    signature: [u8; 8],
    checksum: u8,
    oem_id: [u8; 6],
    revision: u8,
    rsdt_addr: u32,
}

#[derive(Clone, Copy, Debug)]
#[repr(C, packed)]
/// Represents the ACPI 2.0 XSDP.
struct Xsdp2 {
    signature: [u8; 8],
    checksum: u8,
    oem_id: [u8; 6],
    revision: u8,
    rsdt_addr: u32, // Shouldn't be used

    // Extended part
    length: u32,
    xsdt_addr: u64,
    extended_checksum: u8,
    reserved: [u8; 3],
}

#[derive(Debug)]
pub struct Rsdp {
    start_vaddr: VirtAddr,
    revision: AcpiRevision,
    _physical_mapping: PhysicalMapping,
}

impl Rsdp {
    /// Map, validate, read and unmap the RSDP.
    ///
    /// Returns the RSDT address.
    pub fn load(rsdp_paddr: PhysAddr) -> Self {
        let flags = PageTableFlags::PRESENT | PageTableFlags::NO_EXECUTE;

        // First, we need to map the RSDP address to read the data.
        // Otherwise, a nice page fault will occur.
        let physical_mapping = PhysicalMapping::new(rsdp_paddr, size_of::<Xsdp2>(), flags);

        let rsdp_vaddr = physical_mapping.translate(rsdp_paddr).unwrap();

        let rsdp_revision = unsafe {
            rsdp_vaddr
                .as_ptr::<u8>()
                .add(offset_of!(Rsdp1, revision))
                .read()
        };

        // Safety:
        // RSDP address is valid thanks to the bootloader.
        // However, the data can be invalid for now, but we need to enter unsafety
        // for a bit to read the data and validate the checksum.
        let rsdp = match rsdp_revision {
            0 => {
                log::warn!(
                    "ACPI 2.0 not supported, falling back to ACPI 1.0. Some features may not be available"
                );
                super::ACPI_REVISION.store(AcpiRevision::V1);
                Self {
                    start_vaddr: rsdp_vaddr,
                    revision: AcpiRevision::V1,
                    _physical_mapping: physical_mapping,
                }
            }
            2 => {
                super::ACPI_REVISION.store(AcpiRevision::V2);
                Self {
                    start_vaddr: rsdp_vaddr,
                    revision: AcpiRevision::V2,
                    _physical_mapping: physical_mapping,
                }
            }
            x => panic!("Unknown RSDP revision: {}", x),
        };

        let mut sum: u8 = 0;
        for i in 0..match rsdp.revision {
            AcpiRevision::V1 => size_of::<Rsdp1>(),
            AcpiRevision::V2 => size_of::<Xsdp2>(),
        } {
            sum = sum.wrapping_add(unsafe { rsdp.start_vaddr.as_ptr::<u8>().add(i).read() });
        }
        assert_eq!(sum, 0, "RSDP checksum is invalid");

        rsdp
    }

    pub fn rsdt_paddr(&self) -> PhysAddr {
        PhysAddr::new(match self.revision {
            AcpiRevision::V1 => {
                u64::from(unsafe { self.start_vaddr.as_ptr::<Rsdp1>().read().rsdt_addr })
            }
            AcpiRevision::V2 => unsafe { self.start_vaddr.as_ptr::<Xsdp2>().read().xsdt_addr },
        })
    }
}
