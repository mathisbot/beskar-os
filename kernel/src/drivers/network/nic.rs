use crate::drivers::pci;

mod e1000e;

pub fn init(network_controller: pci::Device) {
    match (network_controller.vendor_id(), network_controller.id()) {
        // TODO: Add more e1000e network controllers
        (0x8086, 0x10D3) => e1000e::init(network_controller),
        (0x8086, _) => crate::warn!(
            // Most Intel network controllers should be either e1000 or e1000e
            // so they should all be supported :/
            "Unsupported Intel network controller found. ID: {}",
            network_controller.id()
        ),
        (vendor, id) => crate::warn!(
            "Unsupported network controller found. VendorID: {}; ID: {}",
            vendor,
            id
        ),
    }
}

// TODO: Custom error type
pub trait Nic {
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
