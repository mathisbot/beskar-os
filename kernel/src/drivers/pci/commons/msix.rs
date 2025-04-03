//! Mesage Signaled Interrupts eXtended (MSI-X) support.

use core::ptr::NonNull;

use hyperdrive::ptrs::volatile::{ReadWrite, Volatile};

use crate::arch::interrupts::Irq;
use crate::locals;
use crate::mem::page_alloc::pmap::PhysicalMapping;
use beskar_core::arch::commons::paging::{Flags, M4KiB};

use super::super::{PciHandler, commons::CapabilityHeader, iter_capabilities};

use super::PciAddress;

pub struct MsiX {
    capability: MsiXCapability,
    table: Volatile<ReadWrite, TableEntry>,
    pba: Volatile<ReadWrite, u64>,
    _pmap_table: PhysicalMapping,
    _pmap_pba: PhysicalMapping,
}

#[derive(Debug, Clone, Copy)]
struct MsiXCapability {
    base: PciAddress,
    table_size: u16,
    table_bar_nb: u8,
    table_offset: u32,
    pba_bar_nb: u8,
    pba_offset: u32,
}

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
struct MsiX068 {
    _id: u8,
    _next: u8,
    msg_ctrl: u16,
}

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
struct MsiX06c {
    table_offset: u32,
}

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
struct MsiX070 {
    pba: u32,
}

impl MsiX {
    pub fn new(handler: &mut dyn PciHandler, device: &super::Device) -> Option<Self> {
        let msix_cap = MsiXCapability::find(handler, device)?;

        let table_bar = handler.read_bar(device, msix_cap.table_bar_nb);
        let pba_bar = handler.read_bar(device, msix_cap.pba_bar_nb);

        let Some(super::Bar::Memory(table_bar)) = table_bar else {
            panic!("MSI-X: Table BAR is not a memory BAR");
        };
        let Some(super::Bar::Memory(pba_bar)) = pba_bar else {
            panic!("MSI-X: PBA BAR is not a memory BAR");
        };

        let flags = Flags::MMIO_SUITABLE;

        let table_size = usize::from(msix_cap.table_size) * size_of::<TableEntry>();
        let pmap_table = PhysicalMapping::<M4KiB>::new(
            table_bar.base_address() + u64::from(msix_cap.table_offset),
            table_size,
            flags,
        );
        let table_vaddr = pmap_table
            .translate(table_bar.base_address() + u64::from(msix_cap.table_offset))
            .unwrap();

        let pba_size = usize::from(msix_cap.table_size) * size_of::<u64>();
        let pmap_pba = PhysicalMapping::<M4KiB>::new(
            pba_bar.base_address() + u64::from(msix_cap.pba_offset),
            pba_size,
            flags,
        );
        let pba_vaddr = pmap_pba
            .translate(pba_bar.base_address() + u64::from(msix_cap.pba_offset))
            .unwrap();

        Some(Self {
            capability: msix_cap,
            table: Volatile::new(NonNull::new(table_vaddr.as_mut_ptr()).unwrap()),
            pba: Volatile::new(NonNull::new(pba_vaddr.as_mut_ptr()).unwrap()),
            _pmap_table: pmap_table,
            _pmap_pba: pmap_pba,
        })
    }

    pub fn setup_int(&self, vector: Irq, table_idx: u16) {
        assert!(table_idx < self.capability.table_size);
        let endry_ptr = unsafe { self.table.byte_add(usize::from(table_idx) * 16) };

        let lapic_paddr = unsafe { locals!().lapic().force_lock().paddr() };
        let lapic_id = locals!().apic_id(); // TODO: Load balance between APs?

        let msg_addr = lapic_paddr.as_u64() | (u64::from(lapic_id) << 12);

        // Data format:
        // Bits 0-7: Vector
        // Bits 8-10: Delivery mode
        // Bit 11: Edge/Level
        // Bits 12-15: Reserved
        // Bits 16-31: Destination ID (x2APIC ID)
        let msg_data = vector as u32;

        let table = TableEntry {
            msg_addr_low: u32::try_from(msg_addr & 0xFFFF_FFFC).unwrap(),
            msg_addr_high: u32::try_from(msg_addr >> 32).unwrap(),
            msg_data,
            vector_ctrl: 0,
        };

        unsafe { endry_ptr.write(table) };
    }

    pub fn enable(&self, handler: &mut dyn PciHandler) {
        let offset_0x068_addr = self.capability.base;
        let mut offset_0x068 =
            unsafe { core::mem::transmute::<u32, MsiX068>(handler.read_raw(offset_0x068_addr)) };

        offset_0x068.msg_ctrl |= 1 << (31 - 16);
        offset_0x068.msg_ctrl &= !(1 << (30 - 16));
        handler.write_raw(offset_0x068_addr, unsafe {
            core::mem::transmute::<MsiX068, u32>(offset_0x068)
        });
    }
}

impl MsiXCapability {
    #[must_use]
    pub fn find(handler: &mut dyn PciHandler, device: &super::Device) -> Option<Self> {
        let c = iter_capabilities(handler, device).find(|c| c.id() == CapabilityHeader::ID_MSIX)?;

        let offset_0x068_addr = c.pci_addr();
        let offset_0x068 =
            unsafe { core::mem::transmute::<u32, MsiX068>(handler.read_raw(offset_0x068_addr)) };

        let size = offset_0x068.msg_ctrl & 0x7FF;

        let offset_0x06c_addr = PciAddress::new(
            c.pci_addr().sbdf.segment(),
            c.pci_addr().sbdf.bus(),
            c.pci_addr().sbdf.device(),
            c.pci_addr().sbdf.function(),
            c.pci_addr().register_offset + u8::try_from(size_of::<u32>()).unwrap(),
        );
        let offset_0x06c =
            unsafe { core::mem::transmute::<u32, MsiX06c>(handler.read_raw(offset_0x06c_addr)) };

        let table_bar_nb = u8::try_from(offset_0x06c.table_offset & 0b111).unwrap();
        let table_offset = offset_0x06c.table_offset & !0b111;

        let offset_0x070_addr = PciAddress::new(
            c.pci_addr().sbdf.segment(),
            c.pci_addr().sbdf.bus(),
            c.pci_addr().sbdf.device(),
            c.pci_addr().sbdf.function(),
            c.pci_addr().register_offset + 2 * u8::try_from(size_of::<u32>()).unwrap(),
        );
        let offset_0x070 =
            unsafe { core::mem::transmute::<u32, MsiX070>(handler.read_raw(offset_0x070_addr)) };

        let pba_bar_nb = u8::try_from(offset_0x070.pba & 0b111).unwrap();
        let pba_offset = offset_0x070.pba & !0b111;

        Some(Self {
            base: c.pci_addr(),
            table_size: size + 1,
            table_bar_nb,
            table_offset,
            pba_bar_nb,
            pba_offset,
        })
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
struct TableEntry {
    msg_addr_low: u32,
    msg_addr_high: u32,
    msg_data: u32,
    vector_ctrl: u32,
}
