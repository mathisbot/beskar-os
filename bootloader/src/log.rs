use core::fmt::Write;

use hyperdrive::locks::mcs::{MUMcsLock, McsLock};

mod writer;
use writer::ScreenWriter;

use crate::serial::com::{ComNumber, SerialCom};

static SERIAL: McsLock<SerialCom> = McsLock::new(SerialCom::new(ComNumber::Com1));

static SCREEN_LOGGER: MUMcsLock<ScreenWriter> = MUMcsLock::uninit();

pub fn init_serial() {
    SERIAL.with_locked(|serial| {
        serial.init();
    });
}

pub fn init_screen() {
    let screen = ScreenWriter::new();
    SCREEN_LOGGER.init(screen);
}

pub fn log(args: core::fmt::Arguments) {
    SERIAL.with_locked(|serial| {
        serial.write_fmt(args).unwrap();
    });
    SCREEN_LOGGER.try_with_locked(|writer| {
        writer.write_fmt(args).unwrap();
    });
}

#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => {
        #[cfg(debug_assertions)]
        $crate::log::log(format_args!("[DEBUG] {}\n", format_args!($($arg)*)));
    };
}

#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {
        $crate::log::log(format_args!("[INFO ] {}\n", format_args!($($arg)*)));
    };
}

#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => {
        $crate::log::log(format_args!("[WARN ] {}\n", format_args!($($arg)*)));
    };
}

#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {
        $crate::log::log(format_args!("[ERROR] {}\n", format_args!($($arg)*)));
    };
}
