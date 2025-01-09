//! PCI Express (PCIe) support.

mod msi;

pub fn init() {
    let Some(mcfg) = crate::boot::acpi::ACPI.get().unwrap().mcfg() else {
        return;
    };

    for cs in mcfg.configuration_spaces() {
        crate::debug!("PCIe configuration space: {:#?}", cs);
    }
}
