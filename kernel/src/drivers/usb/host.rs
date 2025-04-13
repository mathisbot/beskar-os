use crate::drivers::pci::{self, Bar, Device};
use beskar_core::drivers::DriverResult;

mod xhci;

pub fn init(usb_controllers: &[Device]) -> DriverResult<()> {
    // Filter out xHCI controllers and get their base addresses
    let xhci = usb_controllers
        .iter()
        .filter(|device| device.csp().prog_if() == 0x30)
        .filter_map(|device| {
            if let Some(Bar::Memory(memory_bar)) =
                pci::with_pci_handler(|handler| handler.read_bar(device, 0))
            {
                Some((*device, memory_bar.base_address()))
            } else {
                None
            }
        });

    xhci::init(xhci)
}
