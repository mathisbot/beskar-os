//! Mesage Signaled Interrupts eXtended (MSI-X) support.

use crate::pci::{PciHandler, commons::CapabilityHeader, iter_capabilities};

use super::PciAddress;

pub struct MsiXCapability {
    base: PciAddress,
    size: u16,
    table_bar_nb: u8,
}

pub fn find_msix_cap(
    handler: &mut dyn PciHandler,
    device: &super::Device,
) -> Option<MsiXCapability> {
    let cap = iter_capabilities(handler, device).find(|c| c.id() == CapabilityHeader::ID_MSIX);

    cap.map(|c| {
        let offset_0x068_addr = PciAddress::new(
            c.pci_addr().sbdf.segment(),
            c.pci_addr().sbdf.bus(),
            c.pci_addr().sbdf.device(),
            c.pci_addr().sbdf.function(),
            c.pci_addr().register_offset,
        );
        let offset_0x068 = handler.read_raw(offset_0x068_addr);

        let size = u16::try_from((offset_0x068 >> 16) & 0x7FF).unwrap();

        let offset_0x06c_addr = PciAddress::new(
            c.pci_addr().sbdf.segment(),
            c.pci_addr().sbdf.bus(),
            c.pci_addr().sbdf.device(),
            c.pci_addr().sbdf.function(),
            c.pci_addr().register_offset + u8::try_from(size_of::<u32>()).unwrap(),
        );
        let offset_0x06c = handler.read_raw(offset_0x06c_addr);

        let table_bar_nb = u8::try_from((offset_0x06c >> 8) & 0b111).unwrap();

        MsiXCapability {
            base: c.pci_addr(),
            size: size + 1,
            table_bar_nb,
        }
    })
}
