use crate::{drivers::pci, network::l2::ethernet::MacAddress};

use beskar_core::drivers::{DriverError, DriverResult};

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
            crate::warn!("Multiple network controllers found, using the first one");
        }
        device
    }) else {
        crate::warn!("No network controller found");
        return Err(DriverError::Absent);
    };

    match (network_controller.vendor_id(), network_controller.id()) {
        // TODO: Add more e1000e network controllers
        (0x8086, 0x10D3) => e1000e::init(network_controller),
        (0x8086, _) => {
            crate::warn!(
                // Most Intel network controllers should be either e1000 or e1000e
                // so they should all be supported :/
                "Unsupported Intel network controller found. ID: {}",
                network_controller.id()
            );
            Err(DriverError::Invalid)
        }
        (vendor, id) => {
            crate::warn!(
                "Unsupported network controller found. VendorID: {}; ID: {}",
                vendor,
                id
            );
            Err(DriverError::Invalid)
        }
    }
}

// TODO: Custom error type
pub trait Nic {
    fn mac_address(&self) -> MacAddress;
    fn poll_frame(&self) -> Option<&[u8]>;
    fn send_frame(&self, frame: &[u8]);
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
