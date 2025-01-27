use beskar_core::arch::commons::PhysAddr;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::{debug, info};

static RSDP_PADDR: AtomicU64 = AtomicU64::new(0);
static RSDP_FOUND: AtomicBool = AtomicBool::new(false);

pub fn init() {
    uefi::system::with_config_table(|config_entries| {
        // Look for ACPI 2 XSDP first
        let acpi2_xsdp = config_entries
            .iter()
            .find(|config_entry| config_entry.guid == uefi::table::cfg::ACPI2_GUID);
        if acpi2_xsdp.is_some() {
            info!("ACPI 2.0 XSDP found");
        } else {
            debug!("ACPI 2.0 XSDP not found");
        }

        // If XSDP is not found, fallback to ACPI 1 RSDP
        let rsdp = acpi2_xsdp.or_else(|| {
            config_entries
                .iter()
                .find(|config_entry| config_entry.guid == uefi::table::cfg::ACPI_GUID)
        });
        if acpi2_xsdp.is_none() && rsdp.is_some() {
            info!("ACPI 1.0 RSDP found");
        } else if rsdp.is_none() {
            debug!("ACPI 1.0 RSDP not found neither");
        }

        if let Some(rsdp) = rsdp {
            RSDP_PADDR.store(rsdp.address as u64, Ordering::Relaxed);
            RSDP_FOUND.store(true, Ordering::Release);
        }
    });
}

pub fn rsdp_paddr() -> Option<PhysAddr> {
    if RSDP_FOUND.load(Ordering::Acquire) {
        Some(PhysAddr::new(RSDP_PADDR.load(Ordering::Relaxed)))
    } else {
        None
    }
}
