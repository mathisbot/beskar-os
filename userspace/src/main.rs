#![no_std]
#![no_main]

use beskar_lib::{exit, io::print};

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    exit(1)
}

#[unsafe(no_mangle)]
pub extern "C" fn _start() {
    print("Hello, userspace!");
}
