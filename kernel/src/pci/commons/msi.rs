//! Mesage Signaled Interrupts (MSI) support.

use crate::pci::{PciHandler, commons::CapabilityHeader, iter_capabilities};

use super::PciAddress;

pub struct MsiCapability {
    base: PciAddress,
}

pub fn find_msi_cap(handler: &mut dyn PciHandler, device: &super::Device) -> Option<MsiCapability> {
    let cap = iter_capabilities(handler, device).find(|c| c.id() == CapabilityHeader::ID_MSI);

    cap.map(|c| MsiCapability { base: c.pci_addr() })
}
