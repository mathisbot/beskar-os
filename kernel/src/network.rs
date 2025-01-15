use crate::pci::{Class, with_pci_handler};

mod nic;
mod structs;

pub fn init() {
    let Some(network_controller) = with_pci_handler(|handler| {
        let mut iter = handler
            .devices()
            .iter()
            .filter(|device| device.csp().class() == Class::Network)
            .copied();
        let device = iter.next();
        if iter.next().is_some() {
            crate::warn!("Multiple network controllers found, using the first one");
        }
        device
    }) else {
        crate::warn!("No network controller found");
        return;
    };

    nic::init(network_controller);
}
