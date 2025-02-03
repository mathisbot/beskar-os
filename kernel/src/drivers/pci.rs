//! General PCI handling module.

mod commons;
pub use commons::{Bar, Class, Device, msi, msix};
use commons::{CapabilityHeader, MemoryBarType, PciAddress, RegisterOffset};

use beskar_core::drivers::{DriverError, DriverResult};

mod express;
mod legacy;

pub fn init() -> DriverResult<()> {
    if let Ok(device_count) = express::init() {
        crate::info!("PCIe initialized with {} devices", device_count);
        Ok(())
    } else if let Ok(device_count) = legacy::init() {
        crate::info!("Legacy PCI initialized with {} devices", device_count);
        Ok(())
    } else {
        crate::warn!("No PCI devices found");
        Err(DriverError::Invalid)
    }
}

pub trait PciHandler {
    #[must_use]
    /// Returns the list of devices found by the PCI handler.
    fn devices(&self) -> &[commons::Device];

    #[must_use]
    fn read_raw(&mut self, address: PciAddress) -> u32;

    fn write_raw(&mut self, address: PciAddress, value: u32);

    #[must_use]
    /// Read the raw value from the PCI configuration space
    ///
    /// Bar number must be 0 to 5 (inclusive).
    fn read_bar(&mut self, device: &commons::Device, bar: u8) -> Option<commons::Bar> {
        let bar_reg_offset = match bar {
            0 => RegisterOffset::Bar0,
            1 => RegisterOffset::Bar1,
            2 => RegisterOffset::Bar2,
            3 => RegisterOffset::Bar3,
            4 => RegisterOffset::Bar4,
            5 => RegisterOffset::Bar5,
            _ => return None,
        } as u8;
        let reg = PciAddress::new(
            device.sbdf().segment(),
            device.sbdf().bus(),
            device.sbdf().device(),
            device.sbdf().function(),
            bar_reg_offset,
        );

        let raw_bar = self.read_raw(reg);

        let upper_value = if raw_bar & 1 == 0 // Memory BAR
            && MemoryBarType::try_from((raw_bar >> 1) & 0b11).unwrap() == MemoryBarType::Qword
        {
            let bar_reg_offset = match bar + 1 {
                0 => RegisterOffset::Bar0,
                1 => RegisterOffset::Bar1,
                2 => RegisterOffset::Bar2,
                3 => RegisterOffset::Bar3,
                4 => RegisterOffset::Bar4,
                5 => RegisterOffset::Bar5,
                _ => panic!("PCI: Invalid BAR number"),
            } as u8;
            let bar_reg = PciAddress::new(
                device.sbdf().segment(),
                device.sbdf().bus(),
                device.sbdf().device(),
                device.sbdf().function(),
                bar_reg_offset,
            );

            self.read_raw(bar_reg)
        } else {
            0
        };

        Some(Bar::from_raw(
            u64::from(raw_bar) | (u64::from(upper_value) << 32),
        ))
    }
}

pub fn iter_capabilities(
    handler: &mut dyn PciHandler,
    device: &commons::Device,
) -> impl Iterator<Item = CapabilityHeader> {
    let cap_ptr_reg = PciAddress::new(
        device.sbdf().segment(),
        device.sbdf().bus(),
        device.sbdf().device(),
        device.sbdf().function(),
        RegisterOffset::CapabilitiesPointer as u8,
    );
    let mut offset = u8::try_from(handler.read_raw(cap_ptr_reg) & 0xFF).unwrap();
    core::iter::from_fn(move || {
        if offset != 0 {
            let cap_reg = PciAddress::new(
                device.sbdf().segment(),
                device.sbdf().bus(),
                device.sbdf().device(),
                device.sbdf().function(),
                offset,
            );
            let cap = handler.read_raw(cap_reg);
            let capability = CapabilityHeader::new(cap_reg, u16::try_from(cap & 0xFFFF).unwrap());

            offset = capability.next();
            Some(capability)
        } else {
            None
        }
    })
}

pub fn with_pci_handler<T, F: FnOnce(&mut dyn PciHandler) -> T>(f: F) -> T {
    if express::pcie_available() {
        // Safety: PCIe is available, thus the handler is initialized.
        unsafe { express::with_pcie_handler(|h| f(h)).unwrap_unchecked() }
    } else {
        legacy::with_legacy_pci_handler(|h| f(h))
    }
}
