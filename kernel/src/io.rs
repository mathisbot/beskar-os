pub mod storage;
pub mod usb;

pub fn init() {
    usb::init();
}
