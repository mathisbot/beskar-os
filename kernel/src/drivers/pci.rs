use crate::mem::page_alloc::pmap::PhysicalMapping;
use ::pci::{LegacyPciHandler, PciExpressHandler, PciHandler};
use beskar_core::{arch::paging::M2MiB, drivers::DriverResult};
use driver_api::DriverError;
use hyperdrive::locks::mcs::{MUMcsLock, McsLock};

static PCIE_HANDLER: MUMcsLock<PciExpressHandler<PhysicalMapping<M2MiB>>> = MUMcsLock::uninit();
static LEGACY_PCI_HANDLER: McsLock<LegacyPciHandler> = McsLock::new(LegacyPciHandler::new());

pub fn init() -> DriverResult<()> {
    if let Ok(device_count) = init_express() {
        video::info!("PCIe devices found: {}", device_count);
        DriverResult::Ok(())
    } else if let Ok(device_count) = init_legacy() {
        video::info!("Legacy PCI devices found: {}", device_count);
        DriverResult::Ok(())
    } else {
        video::error!("PCI failed to initialize or no PCI devices were found");
        DriverResult::Err(DriverError::Invalid)
    }
}

fn init_express() -> DriverResult<usize> {
    let Some(mcfg) = crate::drivers::acpi::ACPI.get().unwrap().mcfg() else {
        return Err(DriverError::Absent);
    };

    let pcie_handler = PciExpressHandler::new(mcfg.configuration_spaces());
    PCIE_HANDLER.init(pcie_handler);

    let device_count = with_pcie_handler(|handler| {
        handler.update_devices();
        handler.devices().len()
    })
    .unwrap();

    if device_count == 0 {
        Err(DriverError::Invalid)
    } else {
        Ok(device_count)
    }
}

fn init_legacy() -> DriverResult<usize> {
    let device_count = with_legacy_pci_handler(|handler| {
        handler.update_devices();
        handler.devices().len()
    });

    if device_count == 0 {
        Err(DriverError::Invalid)
    } else {
        Ok(device_count)
    }
}

#[inline]
fn with_pcie_handler<T, F: FnOnce(&mut PciExpressHandler<PhysicalMapping<M2MiB>>) -> T>(
    f: F,
) -> Option<T> {
    PCIE_HANDLER.with_locked_if_init(f)
}

fn pcie_available() -> bool {
    PCIE_HANDLER.is_initialized()
}

fn with_legacy_pci_handler<T, F: FnOnce(&mut LegacyPciHandler) -> T>(f: F) -> T {
    LEGACY_PCI_HANDLER.with_locked(f)
}

pub fn with_pci_handler<T, F: FnOnce(&mut dyn PciHandler) -> T>(f: F) -> T {
    if pcie_available() {
        // Safety: PCIe is available, thus the handler is initialized.
        unsafe { with_pcie_handler(|h| f(h)).unwrap_unchecked() }
    } else {
        with_legacy_pci_handler(|h| f(h))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MsiHelper;

impl ::pci::MsiHelper for MsiHelper {
    fn get_lapic_info(core_id: usize) -> Option<(beskar_core::arch::PhysAddr, u8)> {
        let core_locals = crate::locals::get_specific_core_locals(core_id)?;
        let lapic_paddr = unsafe { core_locals.lapic().force_lock() }.paddr();
        let lapic_id = core_locals.apic_id();
        Some((lapic_paddr, lapic_id))
    }
}
