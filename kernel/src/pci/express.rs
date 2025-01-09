//! PCI Express (`PCIe`) support.

use alloc::vec::Vec;
use hyperdrive::locks::mcs::MUMcsLock;
use x86_64::{
    PhysAddr,
    structures::paging::{PageTableFlags, Size2MiB},
};

use crate::{
    boot::acpi::sdt::mcfg::ParsedConfigurationSpace, mem::page_alloc::pmap::PhysicalMapping,
};

use super::commons::{Bar, Device};

mod msi;

static PCIE_HANDLER: MUMcsLock<PciExpressHandler> = MUMcsLock::uninit();

pub struct PciExpressHandler {
    configuration_spaces: &'static [ParsedConfigurationSpace],
    devices: Vec<Device>,
}

pub fn init() {
    let Some(mcfg) = crate::boot::acpi::ACPI.get().unwrap().mcfg() else {
        return;
    };

    let pcie_handler = PciExpressHandler::new(mcfg.configuration_spaces());
    PCIE_HANDLER.init(pcie_handler);

    with_pcie_handler(|handler| {
        handler.update_devices();
        if handler.devices.is_empty() {
            crate::warn!("No PCI Express devices found");
        } else {
            crate::debug!("Found {} PCI Express devices", handler.devices.len());
        }
    })
    .unwrap();
}

impl PciExpressHandler {
    #[must_use]
    #[inline]
    pub const fn new(configuration_spaces: &'static [ParsedConfigurationSpace]) -> Self {
        Self {
            configuration_spaces,
            devices: Vec::new(),
        }
    }

    fn update_devices(&mut self) {
        for cs in self.configuration_spaces {
            let start_paddr = Self::build_paddr(cs.offset(), cs.start_pci_bus_number(), 0, 0);
            let end_paddr = Self::build_paddr(cs.offset(), cs.end_pci_bus_number(), 31, 7);

            let length = usize::try_from(end_paddr.as_u64() - start_paddr.as_u64()).unwrap();

            let flags = PageTableFlags::PRESENT
                | PageTableFlags::WRITABLE
                | PageTableFlags::NO_EXECUTE
                | PageTableFlags::NO_CACHE;
            let pmap = PhysicalMapping::<Size2MiB>::new(start_paddr, length, flags);

            for bus in cs.start_pci_bus_number()..=cs.end_pci_bus_number() {
                for dev in 0..=31 {
                    self.devices.push(Self::scan_device(&pmap, cs.offset(), bus, dev));
                }
            }
        }
    }

    fn scan_device(pmap: &PhysicalMapping<Size2MiB>, offset: u64, bus: u8, dev: u8) -> super::commons::Device {
        // TODO: Scan is performed the same way as for PCI devices
        // except it is done using MMIO and not I/O ports.
        let paddr = Self::build_paddr(offset, bus, dev, 0);

        let vaddr = pmap.translate(paddr).unwrap();
        let vendor = unsafe { vaddr.as_ptr::<u16>().read() };
        if vendor != u16::MAX {
            crate::debug!("PCIe device found with vendor {}", vendor);
        }

        todo!("PCIe device scanning");
    }

    fn build_paddr(offset: u64, bus: u8, dev: u8, func: u8) -> PhysAddr {
        let bus = u64::from(bus);
        let dev = u64::from(dev);
        let func = u64::from(func);

        let paddr = offset + (bus << 20) + (dev << 15) + (func << 12);
        PhysAddr::new(paddr)
    }
}

impl super::PciHandler for PciExpressHandler {
    fn devices(&self) -> &[super::commons::Device] {
        &self.devices
    }

    fn read_bar(
        &mut self,
        device: &super::commons::Device,
        bar: u8,
    ) -> Option<super::commons::Bar> {
        None
    }
}

#[inline]
pub fn with_pcie_handler<T, F: FnOnce(&mut PciExpressHandler) -> T>(f: F) -> Option<T> {
    PCIE_HANDLER.try_with_locked(f)
}

pub fn pcie_available() -> bool {
    PCIE_HANDLER.is_initialized()
}
