use crate::pci::Bar;

mod xhci;

pub fn init(usb_controllers: &[crate::pci::Device]) {
    // Filter out xHCI controllers and get their base addresses
    let xhci_paddrs = usb_controllers
        .iter()
        .filter(|device| device.csp().prog_if() == 0x30)
        .filter_map(|device| {
            if let Some(Bar::Memory(memory_bar)) =
                crate::pci::with_pci_handler(|handler| handler.read_bar(device, 0))
            {
                Some(memory_bar.base_address())
            } else {
                None
            }
        });

    xhci::init(xhci_paddrs);
}
