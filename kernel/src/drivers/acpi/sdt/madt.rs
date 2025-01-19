use crate::arch::commons::PhysAddr;
use alloc::vec::Vec;

use super::{Sdt, SdtHeader};

crate::impl_sdt!(Madt);

pub struct ParsedMadt {
    // Related to Local APIC
    lapic_paddr: PhysAddr,

    // Related to I/O APIC
    io_apics: Vec<ParsedIoApic>,
    io_nmi_sources: Vec<ParsedIoNmiSource>,
    io_iso: Vec<ParsedIoIso>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParsedIoApic {
    id: u8,
    addr: PhysAddr,
    gsi_base: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParsedIoNmiSource {
    flags: InterruptFlags,
    gsi: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParsedIoIso {
    source: u8,
    gsi: u32,
    flags: InterruptFlags,
}

#[derive(Debug, Clone, Copy)]
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
    /// The global system interrupt number where this I/O APIC’s interrupt inputs start.
    /// The number of interrupt inputs is determined by the I/O APIC’s Max Redir Entry register.
    gsi_base: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InterruptFlags(u16);

impl InterruptFlags {
    #[must_use]
    #[inline]
    pub fn active_low(self) -> bool {
        // Polarity flag is 2-bit wide, but only 01 (high) and 11 (low) are handled.
        assert_eq!(self.0 & 0b01, 1, "Unexpected polarity flag.");
        self.0 & 0b10 != 0
    }

    #[must_use]
    #[inline]
    pub fn level_triggered(self) -> bool {
        // Trigger mode flag is 2-bit wide, but only 01 (edge) and 11 (level) are handled.
        assert_eq!(self.0 & 0b0100, 4, "Unexpected polarity flag.");
        self.0 & 0b1000 != 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C, packed)]
/// MADT Entry type 2: I/O APIC Interrupt Source Override
struct InterruptSourceOverride {
    header: EntryHeader,
    /// Should always be 0
    bus_source: u8,
    /// Bus-relative IRQ source
    irq_source: u8,
    /// The GSI that the IRQ source will signal
    gsi: u32,
    flags: InterruptFlags,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C, packed)]
/// MADT Entry type 3: I/O APIC Non-maskable interrupt source
///
/// This entry specifies which I/O APIC interrupt inputs should be enabled as non-maskable
struct IoNmiSource {
    header: EntryHeader,
    flags: InterruptFlags,
    /// The GSI that the NMI source will signal
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
    /// Local APIC interrupt input `LINTn` to which NMI is connected.
    lint: u8,
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
/// MADT Entry type 9: Processor Local x2APIC
struct X2Apic {
    header: EntryHeader,
    _reserved: u16,
    id: u32,
    flags: u32,
    acpi_id: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C, packed)]
/// MADT Entry type 10: Local x2APIC NMI Structure
struct X2ApicNmi {
    header: EntryHeader,
    flags: InterruptFlags,
    /// UID corresponding to the ID listed in the processor Device object.
    /// A value of `0xFFFF_FFFF` signifies that this applies to all processors in the machine.
    acpi_uid: u32,
    /// Local x2APIC interrupt input `LINTn` to which NMI is connected.
    lx2apic_lint: u8,
    reserved: [u8; 3],
}

// See <https://uefi.org/htmlspecs/ACPI_Spec_6_4_html/05_ACPI_Software_Programming_Model/ACPI_Software_Programming_Model.html#multiple-apic-description-table-madt>
impl Madt {
    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub fn parse(&self) -> ParsedMadt {
        let mut lapic_paddr = PhysAddr::new(u64::from(unsafe {
            self.start_vaddr
                .as_ptr::<u32>()
                .byte_add(core::mem::offset_of!(MadtHeader, lapic_paddr))
                .read_unaligned()
        }));

        let mut io_apics = Vec::<ParsedIoApic>::new();
        let mut io_nmi_sources = Vec::<ParsedIoNmiSource>::new();
        let mut io_iso = Vec::<ParsedIoIso>::new();

        let madt_header_end = unsafe {
            self.start_vaddr
                .as_ptr::<EntryHeader>()
                .byte_add(size_of::<MadtHeader>())
        };
        let mut offset = 0;
        while offset + size_of::<MadtHeader>() + size_of::<EntryHeader>()
            <= usize::try_from(self.length()).unwrap()
        {
            let entry_start = unsafe { madt_header_end.byte_add(offset) };
            let entry_header = unsafe { entry_start.read_unaligned() };

            match entry_header.entry_type {
                0 => {
                    assert_eq!(usize::from(entry_header.length), size_of::<Lapic>());
                    // Local APIC
                    // This entry could be used to find the ACPI ID of the core, matching by APIC ID.
                }
                1 => {
                    assert_eq!(usize::from(entry_header.length), size_of::<IoApic>());

                    let io_apic = unsafe { entry_start.cast::<IoApic>().read_unaligned() };

                    // Unpack packed fields
                    let id = io_apic.id;
                    let addr = PhysAddr::new(u64::from(io_apic.addr));
                    let gsi_base = io_apic.gsi_base;

                    io_apics.push(ParsedIoApic { id, addr, gsi_base });
                }
                2 => {
                    assert_eq!(
                        usize::from(entry_header.length),
                        size_of::<InterruptSourceOverride>()
                    );

                    let iso = unsafe {
                        entry_start
                            .cast::<InterruptSourceOverride>()
                            .read_unaligned()
                    };
                    assert_eq!(iso.bus_source, 0, "ISO bus source must be 0.");

                    // Unpack packed fields
                    let irq_source = iso.irq_source;
                    let gsi = iso.gsi;
                    let flags = iso.flags;

                    io_iso.push(ParsedIoIso {
                        source: irq_source,
                        gsi,
                        flags,
                    });
                }
                3 => {
                    assert_eq!(usize::from(entry_header.length), size_of::<IoNmiSource>());

                    let nmi_sources = unsafe { entry_start.cast::<IoNmiSource>().read_unaligned() };

                    // Unpack packed fields
                    let flags = nmi_sources.flags;
                    let gsi = nmi_sources.gsi;

                    io_nmi_sources.push(ParsedIoNmiSource { flags, gsi });
                }
                4 => {
                    assert_eq!(usize::from(entry_header.length), size_of::<LocalNmi>());

                    let local_nmi = unsafe { entry_start.cast::<LocalNmi>().read_unaligned() };
                    // TODO: Handle Local NMI.
                    crate::warn!("Unhandled Local NMI entry: {:?}", local_nmi);
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
                // SAPIC related entries
                x if (6..=8).contains(&x) => {
                    unreachable!("PA-RISC architecture specific MADT entry found.")
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
                10 => {
                    assert_eq!(
                        usize::from(entry_header.length),
                        size_of::<X2ApicNmi>(),
                        "Invalid MADT entry length for Local x2APIC NMI Structure."
                    );

                    let x2apic_nmi = unsafe { entry_start.cast::<X2ApicNmi>().read_unaligned() };
                    // TODO: Handle Local x2APIC NMI Structure.
                    crate::warn!(
                        "Unhandled Local x2APIC NMI Structure entry: {:?}",
                        x2apic_nmi
                    );
                }
                // GIC related entries
                x if (11..=15).contains(&x) => {
                    unreachable!("ARM architecture specific MADT entry found.")
                }
                _ => {
                    // We shouldn't panic here
                    crate::warn!(
                        "Unknown MADT entry type: {}, skipping.",
                        entry_header.entry_type
                    );
                }
            }

            offset += usize::from(entry_header.length);
        }

        io_apics.shrink_to_fit();
        io_nmi_sources.shrink_to_fit();
        io_iso.shrink_to_fit();

        ParsedMadt {
            lapic_paddr,
            io_apics,
            io_nmi_sources,
            io_iso,
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
    pub fn io_apics(&self) -> &[ParsedIoApic] {
        &self.io_apics
    }

    #[must_use]
    #[inline]
    pub fn io_nmi_sources(&self) -> &[ParsedIoNmiSource] {
        &self.io_nmi_sources
    }

    #[must_use]
    #[inline]
    pub fn io_iso(&self) -> &[ParsedIoIso] {
        &self.io_iso
    }
}

impl ParsedIoApic {
    #[must_use]
    #[inline]
    pub const fn id(&self) -> u8 {
        self.id
    }

    #[must_use]
    #[inline]
    pub const fn addr(&self) -> PhysAddr {
        self.addr
    }

    #[must_use]
    #[inline]
    pub const fn gsi_base(&self) -> u32 {
        self.gsi_base
    }
}

impl ParsedIoIso {
    #[must_use]
    #[inline]
    pub const fn source(&self) -> u8 {
        self.source
    }

    #[must_use]
    #[inline]
    pub const fn gsi(&self) -> u32 {
        self.gsi
    }

    #[must_use]
    #[inline]
    pub const fn flags(&self) -> InterruptFlags {
        self.flags
    }
}

impl ParsedIoNmiSource {
    #[must_use]
    #[inline]
    pub const fn flags(&self) -> InterruptFlags {
        self.flags
    }

    #[must_use]
    #[inline]
    pub const fn gsi(&self) -> u32 {
        self.gsi
    }
}
