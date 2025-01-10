use alloc::vec::Vec;

use crate::pci::{Class, with_pci_handler};

pub fn init() {
    let display_devices = with_pci_handler(|handler| {
        handler
            .devices()
            .iter()
            .filter(|device| device.csp().class() == Class::Display)
            .copied()
            .collect::<Vec<_>>()
    });

    for device in display_devices {
        crate::debug!("Found display device: {:?}", device);
    }
}
