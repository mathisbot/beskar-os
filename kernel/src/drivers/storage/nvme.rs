use crate::drivers::pci::Device;

pub fn init(nvme: &[Device]) {
    // TODO: Support for multiple NVMe?
    let Some(nvme) = nvme.first() else {
        return;
    };

    crate::debug!("NVMe controller found");
}
