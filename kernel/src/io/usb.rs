use crate::pci::Class;

use alloc::vec::Vec;

pub mod device;
pub mod driver;
pub mod host;
pub mod hub;

pub fn init() {
    // Get all USB controllers from PCI
    let usb_controllers = crate::pci::with_pci_handler(|handler| {
        handler
            .devices()
            .iter()
            .filter(|device| {
                device.csp().class() == Class::SerialBus && device.csp().subclass() == 0x03
            })
            .copied()
            .collect::<Vec<_>>()
    });

    host::init(&usb_controllers);
}
