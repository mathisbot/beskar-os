// FIXME: <https://wiki.osdev.org/AHCI#Determining_what_mode_the_controller_is_in>
use crate::mem::page_alloc::pmap::PhysicalMapping;
use ::pci::{Bar, Device};
use beskar_core::{
    arch::{VirtAddr, paging::M4KiB},
    drivers::{DriverError, DriverResult},
};
use beskar_hal::paging::page_table::Flags;

pub fn init(ahci_controllers: &[Device]) -> DriverResult<()> {
    // TODO: Support for multiple AHCI controllers?
    let Some(controller) = ahci_controllers.first() else {
        return Err(DriverError::Absent);
    };

    let Some(Bar::Memory(bar)) =
        crate::drivers::pci::with_pci_handler(|handler| handler.read_bar(controller, 5))
    else {
        unreachable!();
    };

    let ahci_paddr = bar.base_address();

    let flags = Flags::MMIO_SUITABLE;
    let pmap = PhysicalMapping::<M4KiB>::new(ahci_paddr, 64, flags);

    let ahci_base = pmap.translate(ahci_paddr).unwrap();

    let ahci = Ahci {
        base: ahci_base,
        pmap,
    };

    // TODO: Implement AHCI initialization

    video::debug!("AHCI controller found");

    Ok(())
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(u8)]
enum FisType {
    RegisterHostToDevice = 0x27,
    RegisterDeviceToHost = 0x34,
    DmaActivate = 0x39,
    DmaSetup = 0x41,
    Data = 0x46,
    Bist = 0x58,
    PioSetup = 0x5F,
    SetDeviceBits = 0xA1,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(u8)]
enum AtaCommand {
    IdentifyDevice = 0xEC,
    ReadSector = 0x20,
    WriteSector = 0x30,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(C, packed)]
struct FisH2D {
    fis_type: FisType, // Must be FisType::RegisterHostToDevice
    /// Bits 0-3: Port multiplier
    /// Bits 4-6: Reserved
    /// Bit 7: 1-Command 0-Control
    pmport_c: u8,
    command: AtaCommand,
    feature_l: u8,
    lba0: u8,
    lba1: u8,
    lba2: u8,
    device: u8,
    lba3: u8,
    lba4: u8,
    lba5: u8,
    feature_h: u8,
    count_l: u8,
    count_h: u8,
    icc: u8,
    control: u8,
    _reserved: [u8; 4],
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(C, packed)]
struct FisD2H {
    fis_type: FisType, // Must be FisType::RegisterDeviceToHost
    /// Bits 0-3: Port multiplier
    /// Bits 4-5: Reserved
    /// Bit 6: Interrupt
    /// Bit 7: Reserved
    pmport: u8,
    status: u8,
    error: u8,
    lba0: u8,
    lba1: u8,
    lba2: u8,
    device: u8,
    lba3: u8,
    lba4: u8,
    lba5: u8,
    _reserved1: u8,
    count_l: u8,
    count_h: u8,
    _reserved2: [u8; 6],
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(C, packed)]
struct DataFis {
    fis_type: FisType, // Must be FisType::Data
    /// Bits 0-3: Port multiplier
    /// Bits 4-7: Reserved
    pmport: u8,
    _reserved: [u8; 2],
    /// Data, variable length
    data: [u8; 1],
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(C, packed)]
struct PioSetup {
    fis_type: FisType, // Must be FisType::PioSetup
    /// Bits 0-3: Port multiplier
    /// Bits 4: Reserved
    /// Bit 5: Data transfer direction (1-D2H, 0-H2D)
    /// Bit 6: Interrupt
    /// Bit 7: Reserved
    pmport: u8,
    status: u8,
    error: u8,
    lba0: u8,
    lba1: u8,
    lba2: u8,
    device: u8,
    lba3: u8,
    lba4: u8,
    lba5: u8,
    _reserved1: u8,
    count_l: u8,
    count_h: u8,
    _reserved2: u8,
    e_status: u8,
    transfer_count: u16,
    _reserved3: [u8; 2],
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(C, packed)]
struct DmaSetup {
    fis_type: FisType, // Must be FisType::DmaSetup
    /// Bits 0-3: Port multiplier
    /// Bits 4: Reserved
    /// Bit 5: Data transfer direction (1-D2H, 0-H2D)
    /// Bit 6: Interrupt
    /// Bit 7: Auto-activate
    pmport: u8,
    _reserved1: [u8; 2],
    dma_buffer_id: u64,
    _reserved2: [u8; 4],
    /// First 2 bits must be 0
    dma_buffer_offset: u32,
    /// First bit must be 0
    transfer_count: u32,
    _reserved3: [u8; 4],
}

pub struct Ahci {
    base: VirtAddr,
    pmap: PhysicalMapping,
}
