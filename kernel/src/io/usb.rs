use crate::pci::{Bar, Class};

use alloc::vec::Vec;

pub mod device;
pub mod driver;
pub mod host;
pub mod hub;

pub fn init() {
    // Get all USB devices from PCI
    let usb_devices = crate::pci::with_pci_handler(|handler| {
        handler
            .devices()
            .iter()
            .filter(|device| {
                device.csp().class() == Class::SerialBus && device.csp().subclass() == 0x03
            })
            .copied()
            .collect::<Vec<_>>()
    });

    // Filter out xHCI controllers and get their base addresses
    let xhci_paddrs = usb_devices
        .iter()
        .filter(|device| device.csp().prog_if() == 0x30)
        .filter_map(|device| {
            if let Some(Bar::Memory(memory_bar)) = device.bar(0) {
                Some(memory_bar.base_address())
            } else {
                None
            }
        });

    host::init(xhci_paddrs);
}
