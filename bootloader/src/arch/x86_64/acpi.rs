use beskar_core::arch::commons::PhysAddr;
use hyperdrive::once::Once;

use crate::{info, warn};

static RSDP_PADDR: Once<PhysAddr> = Once::uninit();

pub fn init() {
    if let Some(paddr) = uefi::system::with_config_table(|config_entries| {
        config_entries
            .iter()
            .find(|config_entry| config_entry.guid == uefi::table::cfg::ACPI2_GUID)
            .map_or_else(
                || {
                    config_entries
                        .iter()
                        .find(|config_entry| config_entry.guid == uefi::table::cfg::ACPI_GUID)
                        .map_or_else(
                            || {
                                warn!("ACPI RSDP not found");
                                None
                            },
                            |paddr| {
                                info!("ACPI 1 RSDP found");
                                Some(PhysAddr::new(paddr.address as u64))
                            },
                        )
                },
                |paddr| {
                    info!("ACPI 2 XSDP found");
                    Some(PhysAddr::new(paddr.address as u64))
                },
            )
    }) {
        RSDP_PADDR.call_once(|| paddr);
    }
}

pub fn rsdp_paddr() -> Option<PhysAddr> {
    RSDP_PADDR.get().copied()
}
