//! PCI Express (`PCIe`) support.

use alloc::vec::Vec;
use hyperdrive::locks::mcs::MUMcsLock;
use x86_64::{
    PhysAddr,
    structures::paging::{PageTableFlags, Size2MiB},
};

use crate::{
    boot::acpi::sdt::mcfg::ParsedConfigurationSpace, mem::page_alloc::pmap::PhysicalMapping,
    pci::Class,
};

use super::commons::{
    Bar, BdfAddress, ConfigAddressValue, Csp, Device, MemoryBarType, RegisterOffset,
};

mod msix;

static PCIE_HANDLER: MUMcsLock<PciExpressHandler> = MUMcsLock::uninit();

pub struct PciExpressHandler {
    configuration_spaces: &'static [ParsedConfigurationSpace],
    physical_mappings: Vec<PhysicalMapping<Size2MiB>>,
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
    pub fn new(configuration_spaces: &'static [ParsedConfigurationSpace]) -> Self {
        let physical_mappings = configuration_spaces
            .iter()
            .map(|cs| {
                let length = usize::try_from(
                    cs.address_range().end().as_u64() - cs.address_range().start().as_u64(),
                )
                .unwrap();

                let flags = PageTableFlags::PRESENT
                    | PageTableFlags::WRITABLE
                    | PageTableFlags::NO_EXECUTE
                    | PageTableFlags::NO_CACHE;
                PhysicalMapping::<Size2MiB>::new(*cs.address_range().start(), length, flags)
            })
            .collect::<Vec<_>>();

        Self {
            configuration_spaces,
            physical_mappings,
            devices: Vec::new(),
        }
    }

    fn update_devices(&mut self) {
        self.devices.clear();

        // Brute-force scan
        for (cs, pmap) in self
            .configuration_spaces
            .iter()
            .zip(&self.physical_mappings)
        {
            for bus in cs.start_pci_bus_number()..=cs.end_pci_bus_number() {
                for dev in 0..=31 {
                    if let Some(device) = Self::scan_device(
                        pmap,
                        cs,
                        ConfigAddressValue::new(bus, dev, 0, RegisterOffset::VendorId as u8),
                    ) {
                        self.devices.push(device);
                    }
                }
            }
        }
    }

    fn scan_device(
        pmap: &PhysicalMapping<Size2MiB>,
        cs: &ParsedConfigurationSpace,
        address: ConfigAddressValue,
    ) -> Option<Device> {
        let (device, vendor) = {
            let reg = ConfigAddressValue {
                register_offset: RegisterOffset::VendorId as u8,
                ..address
            };
            let paddr = Self::build_paddr(cs.offset(), reg);
            let vaddr = pmap.translate(paddr)?;
            let value = unsafe { vaddr.as_ptr::<u32>().read() };

            if value & 0xFFFF == u32::from(u16::MAX) {
                return None;
            }

            (
                u16::try_from(value >> 16).unwrap(),
                u16::try_from(value & 0xFFFF).unwrap(),
            )
        };

        let (class, subclass, prog_if, revision) = {
            let reg = ConfigAddressValue {
                register_offset: RegisterOffset::RevisionId as u8,
                ..address
            };
            let paddr = Self::build_paddr(cs.offset(), reg);
            let vaddr = pmap.translate(paddr)?;
            let value = unsafe { vaddr.as_ptr::<u32>().read() };
            (
                Class::from(u8::try_from(value >> 24).unwrap()),
                u8::try_from((value >> 16) & 0xFF).unwrap(),
                u8::try_from((value >> 8) & 0xFF).unwrap(),
                u8::try_from(value & 0xFF).ok().unwrap(),
            )
        };

        let functions = Self::find_function_count(pmap, cs.offset(), address);

        Some(Device {
            id: device,
            vendor_id: vendor,
            bdf: address.bdf,
            functions,
            csp: Csp::new(class, subclass, prog_if),
            revision,
            segment_group_number: cs.segment_group_number(),
        })
    }

    fn find_function_count(
        pmap: &PhysicalMapping<Size2MiB>,
        offset: u64,
        address: ConfigAddressValue,
    ) -> u8 {
        let multifonction = {
            let reg = ConfigAddressValue {
                register_offset: RegisterOffset::HeaderType as u8,
                ..address
            };
            let paddr = Self::build_paddr(offset, reg);
            let vaddr = pmap.translate(paddr).unwrap();
            let value = unsafe { vaddr.as_ptr::<u8>().read() };

            value >> 7 == 1
        };

        if !multifonction {
            return 1;
        }

        u8::try_from(
            (1..8)
                .filter(|&func| {
                    let reg = ConfigAddressValue {
                        bdf: BdfAddress::new(address.bdf.bus(), address.bdf.device(), func),
                        register_offset: RegisterOffset::VendorId as u8,
                        ..address
                    };
                    let paddr = Self::build_paddr(offset, reg);
                    let vaddr = pmap.translate(paddr).unwrap();

                    // Vendor ID is 0xFFFF if function is unsupported
                    unsafe { vaddr.as_ptr::<u16>().read() != u16::MAX }
                })
                .count(),
        )
        .unwrap()
            + 1
    }

    fn build_paddr(offset: u64, address: ConfigAddressValue) -> PhysAddr {
        let bus = u64::from(address.bdf.bus());
        let dev = u64::from(address.bdf.device());
        let func = u64::from(address.bdf.function());
        let reg = u64::from(address.register_offset);

        let paddr = offset + (bus << 20) + (dev << 15) + (func << 12) + reg;
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
        bar_number: u8,
    ) -> Option<super::commons::Bar> {
        let bar_reg_offset = match bar_number {
            0 => RegisterOffset::Bar0,
            1 => RegisterOffset::Bar1,
            2 => RegisterOffset::Bar2,
            3 => RegisterOffset::Bar3,
            4 => RegisterOffset::Bar4,
            5 => RegisterOffset::Bar5,
            _ => return None,
        } as u8;
        let reg = ConfigAddressValue::new(
            device.bdf().bus(),
            device.bdf().device(),
            device.bdf().function(),
            bar_reg_offset,
        );

        let (cs, pmap) = {
            let cs_index = self
                .configuration_spaces
                .iter()
                .position(|cs| cs.segment_group_number() == device.segment_group_number)
                .unwrap();

            (
                &self.configuration_spaces[cs_index],
                &self.physical_mappings[cs_index],
            )
        };

        let paddr = Self::build_paddr(cs.offset(), reg);
        let vaddr = pmap.translate(paddr).unwrap();

        let bar = unsafe { vaddr.as_ptr::<u32>().read() };
        if bar == u32::MAX {
            return None;
        }

        let upper_value = if bar & 1 == 0 // Memory BAR
            && MemoryBarType::try_from((bar >> 1) & 0b11).unwrap() == MemoryBarType::Qword
        {
            let bar_reg_offset = match bar_number + 1 {
                0 => RegisterOffset::Bar0,
                1 => RegisterOffset::Bar1,
                2 => RegisterOffset::Bar2,
                3 => RegisterOffset::Bar3,
                4 => RegisterOffset::Bar4,
                5 => RegisterOffset::Bar5,
                _ => panic!("PCI: Invalid BAR number"),
            } as u8;
            let bar_reg = ConfigAddressValue::new(
                device.bdf().bus(),
                device.bdf().device(),
                device.bdf().function(),
                bar_reg_offset,
            );

            let paddr = Self::build_paddr(cs.offset(), bar_reg);
            let vaddr = pmap.translate(paddr).unwrap();

            let bar = unsafe { vaddr.as_ptr::<u32>().read() };
            if bar == u32::MAX {
                return None;
            }
            bar
        } else {
            0
        };

        Some(Bar::from_raw(
            u64::from(bar) | (u64::from(upper_value) << 32),
        ))
    }
}

#[inline]
pub fn with_pcie_handler<T, F: FnOnce(&mut PciExpressHandler) -> T>(f: F) -> Option<T> {
    PCIE_HANDLER.try_with_locked(f)
}

pub fn pcie_available() -> bool {
    PCIE_HANDLER.is_initialized()
}
