use crate::{info, warn};
use beskar_core::arch::PhysAddr;
use hyperdrive::once::Once;
use uefi::table::cfg::ConfigTableEntry;

static RSDP_PADDR: Once<PhysAddr> = Once::uninit();

pub fn init() {
    if let Some(paddr) = uefi::system::with_config_table(|config_entries| {
        find_guid(config_entries, ConfigTableEntry::ACPI2_GUID)
            .or_else(|| find_guid(config_entries, ConfigTableEntry::ACPI_GUID))
    }) {
        RSDP_PADDR.call_once(|| paddr);
        info!("ACPI RSDP found at physical address: {:#x}", paddr.as_u64());
    } else {
        warn!("ACPI RSDP not found");
    }
}

fn find_guid(config_entries: &[ConfigTableEntry], guid: uefi::Guid) -> Option<PhysAddr> {
    config_entries
        .iter()
        .find(|config_entry| config_entry.guid == guid)
        .map(|paddr| PhysAddr::new(paddr.address as u64))
}

pub fn rsdp_paddr() -> Option<PhysAddr> {
    RSDP_PADDR.get().copied()
}
