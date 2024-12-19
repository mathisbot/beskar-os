use crate::pci::Class;

pub mod device;
pub mod driver;
pub mod host;
pub mod hub;

pub fn init() {
    let usb_device = crate::pci::with_pci_handler(|handler| {
        let mut usb_device = None;
        for device in handler.devices() {
            if device.csp().class() == Class::SerialBus && device.csp().subclass() == 0x03 {
                usb_device = Some(device);
                break;
            }
        }
        usb_device.cloned()
    });

    let Some(usb_device) = usb_device else {
        log::warn!("No USB device found");
        return;
    };

    if usb_device.csp().prog_if() != 0x30 {
        log::warn!("USB device is not an xHCI controller");
        return;
    }

    log::debug!("xHCI controller found: {:?}", usb_device);

    let xhci_bar0 = usb_device.bar(0).unwrap();
    log::debug!("xHCI BAR0: {:?}", xhci_bar0);
}
