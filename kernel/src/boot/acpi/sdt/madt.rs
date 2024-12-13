#![allow(dead_code, unused_variables)] // TODO: Remove

use core::mem::offset_of;

use x86_64::PhysAddr;

use crate::impl_sdt;

use super::{Sdt, SdtHeader};

impl_sdt!(Madt);

pub struct ParsedMadt {
    // Related to Local APIC
    lapic_paddr: PhysAddr,

    // Related to I/O APIC
    io_apic_id: Option<u8>,
    io_apic_addr: Option<u32>,
    gsi_base: Option<u32>,
}

#[repr(C, packed)]
struct MadtHeader {
    sdt_header: SdtHeader,
    lapic_paddr: u32,
    flags: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C, packed)]
struct EntryHeader {
    entry_type: u8,
    length: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C, packed)]
/// MADT Entry type 0: Local APIC
struct Lapic {
    header: EntryHeader,
    acpi_id: u8,
    id: u8,
    flags: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C, packed)]
/// MADT Entry type 1: I/O APIC
struct IoApic {
    header: EntryHeader,
    id: u8,
    _reserved: u8,
    addr: u32,
    gsi_base: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct InterruptFlags(u16);

impl InterruptFlags {
    #[must_use]
    #[inline]
    pub const fn active_low(self) -> bool {
        self.0 & 2 != 0
    }

    #[must_use]
    #[inline]
    pub const fn level_triggered(self) -> bool {
        self.0 & 8 != 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C, packed)]
/// MADT Entry type 2: I/O APIC Interrupt Source Override
struct InterruptSourceOverride {
    header: EntryHeader,
    bus_source: u8,
    irq_source: u8,
    gsi: u32,
    flags: InterruptFlags,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C, packed)]
/// MADT Entry type 3: I/O APIC Non-maskable interrupt source
struct IoNmiSource {
    header: EntryHeader,
    nmi_source: u8,
    _reserved: u8,
    flags: InterruptFlags,
    gsi: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C, packed)]
/// MADT Entry type 4: Local APIC Non-maskable interrupts
struct LocalNmi {
    header: EntryHeader,
    /// 0xFF means all CPUs
    acpi_id: u8,
    flags: InterruptFlags,
    lint: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C, packed)]
/// MADT Entry type 5: Local APIC Address Override
struct LapicAddressOverride {
    header: EntryHeader,
    _reserved: u16,
    address: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C, packed)]
/// MADT Entry type 6: I/O SAPIC
struct IoSapic {
    header: EntryHeader,
    id: u8,
    _reserved: u8,
    gsi_base: u32,
    addr: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C, packed)]
/// MADT Entry type 9: Processor Local x2APIC
struct X2Apic {
    header: EntryHeader,
    _reserved: u16,
    id: u32,
    flags: u32,
    acpi_id: u32,
}

// See <https://uefi.org/htmlspecs/ACPI_Spec_6_4_html/05_ACPI_Software_Programming_Model/ACPI_Software_Programming_Model.html#multiple-apic-description-table-madt>
impl Madt {
    #[must_use]
    pub fn parse(&self) -> ParsedMadt {
        let mut lapic_paddr = PhysAddr::new(u64::from(unsafe {
            self.start_vaddr
                .as_ptr::<u32>()
                .byte_add(offset_of!(MadtHeader, lapic_paddr))
                .read_unaligned()
        }));
        let mut io_apic_id = None;
        let mut io_apic_addr = None;
        let mut gsi_base = None;

        let entry_start = unsafe {
            self.start_vaddr
                .as_ptr::<EntryHeader>()
                .byte_add(size_of::<MadtHeader>())
        };
        let mut offset = 0;
        while offset + size_of::<MadtHeader>() + size_of::<EntryHeader>()
            < usize::try_from(self.length()).unwrap()
        {
            let entry_header = unsafe { entry_start.byte_add(offset).read_unaligned() };

            match entry_header.entry_type {
                0 => {
                    assert_eq!(usize::from(entry_header.length), size_of::<Lapic>());
                    // Local APIC
                    // This entry could be used to find the ACPI ID of the core, matching by APIC ID.
                }
                1 => {
                    assert_eq!(usize::from(entry_header.length), size_of::<IoApic>());

                    let io_apic = unsafe { entry_start.cast::<IoApic>().read_unaligned() };

                    let old_io_apic_id = io_apic_id.replace(io_apic.id);
                    let old_io_apic_addr = io_apic_addr.replace(io_apic.addr);
                    let old_gsi_base = gsi_base.replace(io_apic.gsi_base);
                    if old_io_apic_id.is_some()
                        || old_io_apic_addr.is_some()
                        || old_gsi_base.is_some()
                    {
                        log::warn!("Multiple I/O APICs are not supported YET. Using first found.");
                        io_apic_id = old_io_apic_id;
                        io_apic_addr = old_io_apic_addr;
                        gsi_base = old_gsi_base;
                    }
                }
                2 => {
                    assert_eq!(
                        usize::from(entry_header.length),
                        size_of::<InterruptSourceOverride>()
                    );

                    let interrupt_source_override = unsafe {
                        entry_start
                            .cast::<InterruptSourceOverride>()
                            .read_unaligned()
                    };

                    // TODO: Understand what this entry type does.
                    log::warn!(
                        "I/O APIC Interrupt Source Override entry type found but not implemented."
                    );

                    assert_eq!(
                        interrupt_source_override.bus_source, 0,
                        "ISO bus source must be 0."
                    );

                    // Mandatory to unwrap because struct is unaligned.
                    let irq_source = interrupt_source_override.irq_source;
                    let gsi = interrupt_source_override.gsi;
                    let flags = interrupt_source_override.flags;

                    log::warn!(
                        "irq_source: {}, gsi: {}, flags: {:?}",
                        irq_source,
                        gsi,
                        flags
                    );
                }
                3 => {
                    assert_eq!(usize::from(entry_header.length), size_of::<IoNmiSource>());

                    let nmi_sources = unsafe { entry_start.cast::<IoNmiSource>().read_unaligned() };
                    // TODO: Handle NMI sources.
                }
                4 => {
                    assert_eq!(usize::from(entry_header.length), size_of::<LocalNmi>());

                    let local_nmi = unsafe { entry_start.cast::<LocalNmi>().read_unaligned() };
                    // TODO: Handle Local NMI.
                }
                5 => {
                    assert_eq!(
                        usize::from(entry_header.length),
                        size_of::<LapicAddressOverride>(),
                        "Invalid MADT entry length for Local APIC Address Override."
                    );

                    let lapic_override =
                        unsafe { entry_start.cast::<LapicAddressOverride>().read_unaligned() };
                    lapic_paddr = PhysAddr::new(lapic_override.address);
                }
                6 => {
                    assert_eq!(
                        usize::from(entry_header.length),
                        size_of::<IoSapic>(),
                        "Invalid MADT entry length for I/O SAPIC."
                    );
                    // TODO: I/O SAPIC, overrides the I/O APIC
                }
                9 => {
                    assert_eq!(
                        usize::from(entry_header.length),
                        size_of::<X2Apic>(),
                        "Invalid MADT entry length for Processor Local x2APIC."
                    );
                    // Local x2APIC
                    // Same as Local APIC
                }
                _ => {
                    // We shouldn't panic here
                    log::warn!(
                        "Unknown MADT entry type: {}, skipping.",
                        entry_header.entry_type
                    );
                }
            }

            offset += usize::from(entry_header.length);
        }

        ParsedMadt {
            lapic_paddr,

            io_apic_id,
            io_apic_addr,
            gsi_base,
        }
    }
}

impl ParsedMadt {
    #[must_use]
    #[inline]
    pub const fn lapic_paddr(&self) -> PhysAddr {
        self.lapic_paddr
    }

    #[must_use]
    #[inline]
    pub const fn io_apic_id(&self) -> Option<u8> {
        self.io_apic_id
    }

    #[must_use]
    #[inline]
    pub const fn io_apic_addr(&self) -> Option<u32> {
        self.io_apic_addr
    }

    #[must_use]
    #[inline]
    pub const fn gsi_base(&self) -> Option<u32> {
        self.gsi_base
    }
}
