use crate::pci::{Bar, Class};

pub mod device;
pub mod driver;
pub mod host;
pub mod hub;

pub fn init() {
    let Some(usb_device) = crate::pci::with_pci_handler(|handler| {
        handler
            .devices()
            .iter()
            .find(|device| {
                device.csp().class() == Class::SerialBus && device.csp().subclass() == 0x03
            })
            .cloned()
    }) else {
        log::warn!("No USB controller found");
        return;
    };

    if usb_device.csp().prog_if() != 0x30 {
        log::warn!("Non-xHCI USB devices are not supported yet, skipping");
        return;
    }

    // xHCI uses BAR0
    let Some(Bar::Memory(usb_device_bar)) = usb_device.bar(0) else {
        log::warn!("xHCI controller does not have a memory-mapped BAR0");
        return;
    };

    host::init(usb_device_bar.base_address());
}
