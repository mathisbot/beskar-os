use super::pci::Class;
use beskar_core::drivers::DriverResult;

use alloc::vec::Vec;

pub mod device;
pub mod driver;
pub mod host;
pub mod hub;

pub fn init() -> DriverResult<()> {
    // Get all USB controllers from PCI
    let usb_controllers = super::pci::with_pci_handler(|handler| {
        handler
            .devices()
            .iter()
            .filter(|device| {
                device.csp().class() == Class::SerialBus && device.csp().subclass() == 0x03
            })
            .copied()
            .collect::<Vec<_>>()
    });

    host::init(&usb_controllers)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceState {
    /// Immediately after device is attached
    Attached,
    /// Device receives (but not necessarily uses) power
    Powered,
    /// The powered device receives a reset signal
    Default,
    /// The host assigns an address to the device
    Addressed,
    /// The device is configured by the host.
    /// All endpoints data toggles are reset to 0.
    Configured,
    /// The host enters suspended state if no traffic is observed for a period of time
    /// (1 ms). It automatically resumes when traffic is detected, but it should not be
    /// expected to respond in the first 10 ms after resuming.
    Suspended,
}
