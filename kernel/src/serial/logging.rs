use crate::utils::locks::McsLock;
use core::fmt::Write;
use x86_64::instructions::port::Port;

use super::SerialPort;

/// Port number for COM1
const COM1_IO_PORT: u16 = 0x3F8;

#[cfg(debug_assertions)]
static SERIAL_PORT: McsLock<SerialLogger> = McsLock::new(SerialLogger::new());

#[inline]
pub fn init() {
    SERIAL_PORT.with_locked(|serial| {
        serial.init();
    });
}

pub struct SerialLogger(SerialPort);

impl SerialLogger {
    #[must_use]
    #[inline]
    pub const fn new() -> Self {
        Self(SerialPort::new(COM1_IO_PORT))
    }
}

impl core::ops::Deref for SerialLogger {
    type Target = SerialPort;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl core::ops::DerefMut for SerialLogger {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl core::fmt::Write for SerialLogger {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for byte in s.bytes() {
            self.send(byte);
        }
        Ok(())
    }
}

#[cfg_attr(not(debug_assertions), allow(unused_variables))]
pub fn serial_print(args: core::fmt::Arguments) {
    #[cfg(debug_assertions)]
    SERIAL_PORT.with_locked(|serial| {
        serial.write_fmt(args).unwrap();
    });
}

#[macro_export]
macro_rules! serprint {
    ($($arg:tt)*) => {
        #[cfg(debug_assertions)]
        $crate::serial::logging::serial_print(format_args!($($arg)*));
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
