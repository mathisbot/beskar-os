use crate::drivers::pci;
use beskar_core::drivers::{DriverError, DriverResult};
use network::Nic;

mod e1000e;

pub fn init() -> DriverResult<()> {
    let Some(network_controller) = pci::with_pci_handler(|handler| {
        let mut iter = handler
            .devices()
            .iter()
            .filter(|device| device.csp().class() == pci::Class::Network)
            .copied();
        let device = iter.next();
        if iter.next().is_some() {
            video::warn!("Multiple network controllers found, using the first one");
        }
        device
    }) else {
        video::warn!("No network controller found");
        return Err(DriverError::Absent);
    };

    match (network_controller.vendor_id(), network_controller.id()) {
        // TODO: Add more e1000e network controllers
        (0x8086, 0x10D3) => e1000e::init(network_controller),
        (0x8086, _) => {
            video::warn!(
                // Most Intel network controllers should be either e1000 or e1000e
                // so they should all be supported :/
                "Unsupported Intel network controller found. ID: {}",
                network_controller.id()
            );
            Err(DriverError::Invalid)
        }
        (vendor, id) => {
            video::warn!(
                "Unsupported network controller found. VendorID: {}; ID: {}",
                vendor,
                id
            );
            Err(DriverError::Invalid)
        }
    }
}

pub fn with_nic<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut dyn Nic) -> R,
{
    if e1000e::e1000e_available() {
        Some(e1000e::with_e1000e(|nic| f(nic)))
    } else {
        None
    }
}
