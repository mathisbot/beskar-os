#![expect(dead_code, reason = "Drivers are not fully implemented yet")]
pub mod acpi;
pub mod hpet;
pub mod keyboard;
pub mod nic;
mod pci;
pub mod ps2;
pub mod storage;
pub mod tsc;
pub mod usb;

pub extern "C" fn init() -> ! {
    let pci_init_result = pci::init();
    if pci_init_result.is_err() {
        video::warn!("PCI initialization failed");
    }

    // TODO: Start each driver's process when needed

    let _ = keyboard::init();

    #[cfg(target_arch = "x86_64")]
    let _ = ps2::init();

    let _ = storage::init();
    let _ = usb::init();
    let _ = nic::init();

    unsafe { crate::process::scheduler::exit_current_thread() };
}
