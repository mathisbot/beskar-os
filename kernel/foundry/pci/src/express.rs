//! PCI Express (`PCIe`) support.

use acpi::sdt::mcfg::ParsedConfigurationSpace;
use alloc::vec::Vec;
use beskar_core::arch::{PhysAddr, paging::M2MiB};
use beskar_hal::paging::page_table::Flags;
use driver_api::PhysicalMapper;

use super::commons::{Class, Csp, Device, PciAddress, RegisterOffset, SbdfAddress};

pub struct PciExpressHandler<M: PhysicalMapper<M2MiB>> {
    configuration_spaces: &'static [ParsedConfigurationSpace],
    physical_mappings: Vec<M>,
    devices: Vec<Device>,
}

impl<M: PhysicalMapper<M2MiB>> PciExpressHandler<M> {
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

                let flags = Flags::MMIO_SUITABLE;
                M::new(*cs.address_range().start(), length, flags)
            })
            .collect::<Vec<_>>();

        Self {
            configuration_spaces,
            physical_mappings,
            devices: Vec::new(),
        }
    }

    pub fn update_devices(&mut self) {
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
                        PciAddress::new(
                            cs.segment_group_number(),
                            bus,
                            dev,
                            0,
                            RegisterOffset::VendorId as u8,
                        ),
                    ) {
                        self.devices.push(device);
                    }
                }
            }
        }
    }

    fn scan_device(pmap: &M, cs: &ParsedConfigurationSpace, address: PciAddress) -> Option<Device> {
        let (device, vendor) = {
            let reg = PciAddress {
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
            let reg = PciAddress {
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
            sbdf: address.sbdf,
            functions,
            csp: Csp::new(class, subclass, prog_if),
            revision,
            segment_group_number: cs.segment_group_number(),
        })
    }

    fn find_function_count(pmap: &M, offset: u64, address: PciAddress) -> u8 {
        let multifonction = {
            let reg = PciAddress {
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
                    let reg = PciAddress {
                        sbdf: SbdfAddress::new(
                            address.sbdf.segment(),
                            address.sbdf.bus(),
                            address.sbdf.device(),
                            func,
                        ),
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

    fn build_paddr(offset: u64, address: PciAddress) -> PhysAddr {
        let bus = u64::from(address.sbdf.bus());
        let dev = u64::from(address.sbdf.device());
        let func = u64::from(address.sbdf.function());
        let reg = u64::from(address.register_offset);

        let paddr = offset + (bus << 20) + (dev << 15) + (func << 12) + reg;
        PhysAddr::new(paddr)
    }
}

impl<M: PhysicalMapper<M2MiB>> super::PciHandler for PciExpressHandler<M> {
    fn devices(&self) -> &[super::commons::Device] {
        &self.devices
    }

    fn read_raw(&mut self, address: PciAddress) -> u32 {
        let (cs, pmap) = {
            let cs_index = self
                .configuration_spaces
                .iter()
                .position(|cs| cs.segment_group_number() == address.sbdf.segment())
                .unwrap();
            (
                &self.configuration_spaces[cs_index],
                &self.physical_mappings[cs_index],
            )
        };

        let paddr = Self::build_paddr(cs.offset(), address);
        let vaddr = pmap.translate(paddr).unwrap();

        unsafe { vaddr.as_ptr::<u32>().read() }
    }

    fn write_raw(&mut self, address: PciAddress, value: u32) {
        let (cs, pmap) = {
            let cs_index = self
                .configuration_spaces
                .iter()
                .position(|cs| cs.segment_group_number() == address.sbdf.segment())
                .unwrap();
            (
                &self.configuration_spaces[cs_index],
                &self.physical_mappings[cs_index],
            )
        };

        let paddr = Self::build_paddr(cs.offset(), address);
        let vaddr = pmap.translate(paddr).unwrap();

        unsafe { vaddr.as_mut_ptr::<u32>().write(value) };
    }
}
