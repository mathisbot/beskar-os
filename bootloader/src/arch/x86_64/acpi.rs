use beskar_core::arch::commons::PhysAddr;
use hyperdrive::once::Once;

use crate::info;

static RSDP_PADDR: Once<PhysAddr> = Once::uninit();

pub fn init() {
    if let Some(paddr) = uefi::system::with_config_table(|config_entries| {
        // Look for ACPI 2 XSDP first
        if let Some(paddr) = config_entries
            .iter()
            .find(|config_entry| config_entry.guid == uefi::table::cfg::ACPI2_GUID)
        {
            info!("ACPI 2 XSDP found");
            return Some(PhysAddr::new(paddr.address as u64));
        }

        // Look for ACPI 1 RSDP otherwise
        if let Some(paddr) = config_entries
            .iter()
            .find(|config_entry| config_entry.guid == uefi::table::cfg::ACPI_GUID)
        {
            info!("ACPI 1 RSDP found");
            return Some(PhysAddr::new(paddr.address as u64));
        }

        None
    }) {
        RSDP_PADDR.call_once(|| paddr);
    }
}

pub fn rsdp_paddr() -> Option<PhysAddr> {
    RSDP_PADDR.get().copied()
}
