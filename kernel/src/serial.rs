//! Serial port driver for printing debug messages.
//!
//! In QEMU, the serial port can be mapped to the host machine's stdio.
//!
//! On a physical machine, the serial port can be connected to another machine
//! to capture early debug messages in case of hard failure.

#![allow(dead_code)]
#![allow(unused_imports)]

use crate::utils::locks::{MUMcsLock, McsNode};
use core::fmt::Write;
use uart_16550::SerialPort;

/// Port number for COM1
const COM1_IO_PORT: u16 = 0x3F8;

#[cfg(debug_assertions)]
pub static SERIAL_PORT: MUMcsLock<SerialPort> = MUMcsLock::uninit();

#[inline]
pub fn init() {
    #[cfg(debug_assertions)]
    SERIAL_PORT.init(unsafe { SerialPort::new(COM1_IO_PORT) });
}

#[cfg_attr(not(debug_assertions), allow(unused_variables))]
pub fn serial_print(args: core::fmt::Arguments) {
    #[cfg(debug_assertions)]
    SERIAL_PORT.try_with_locked(|serial| {
        serial.write_fmt(args).unwrap();
    });
}

#[macro_export]
macro_rules! serprint {
    ($($arg:tt)*) => {
        #[cfg(debug_assertions)]
        $crate::serial::serial_print(format_args!($($arg)*));
    };
}

#[macro_export]
macro_rules! sererror {
    ($fmt:expr) => ($crate::serprint!(concat!("[ERROR] ", $fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => ($crate::serprint!(
        concat!("[ERROR] ", $fmt, "\n"), $($arg)*));
}

#[macro_export]
macro_rules! serwarn {
    ($fmt:expr) => ($crate::serprint!(concat!("[WARN ] ", $fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => ($crate::serprint!(
        concat!("[WARN ] ", $fmt, "\n"), $($arg)*));
}

#[macro_export]
macro_rules! serinfo {
    ($fmt:expr) => ($crate::serprint!(concat!("[INFO ] ", $fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => ($crate::serprint!(
        concat!("[INFO ] ", $fmt, "\n"), $($arg)*));
}

#[macro_export]
macro_rules! serdebug {
    ($fmt:expr) => ($crate::serprint!(concat!("[DEBUG] ", $fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => ($crate::serprint!(
        concat!("[DEBUG] ", $fmt, "\n"), $($arg)*));
}

#[macro_export]
macro_rules! sertrace {
    ($fmt:expr) => ($crate::serprint!(concat!("[TRACE] ", $fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => ($crate::serprint!(
        concat!("[TRACE] ", $fmt, "\n"), $($arg)*));
}
