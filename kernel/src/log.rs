use core::{fmt::Write, sync::atomic::AtomicBool};
use hyperdrive::locks::mcs::{MUMcsLock, McsLock};

use crate::serial::com::{ComNumber, SerialCom};

mod writer;
use writer::ScreenWriter;

static SERIAL: McsLock<SerialCom> = McsLock::new(SerialCom::new(ComNumber::Com1));

static LOG_ON_SCREEN: AtomicBool = AtomicBool::new(false);
static SCREEN_LOGGER: MUMcsLock<ScreenWriter> = MUMcsLock::uninit();

/// Initialize the serial logger.
///
/// This function should be called at the very beginning of the kernel.
pub fn init_serial() {
    SERIAL.with_locked(|serial| {
        serial.init();
    });
}

/// Initialize the screen logger.
///
/// This function should be called after the screen has been initialized.
pub fn init_screen() {
    let screen = ScreenWriter::new();
    SCREEN_LOGGER.init(screen);
    set_screen_logging(true);
}

pub fn set_screen_logging(enable: bool) {
    LOG_ON_SCREEN.store(enable, core::sync::atomic::Ordering::Relaxed);
}

pub fn log(args: core::fmt::Arguments) {
    SERIAL.with_locked(|serial| {
        serial.write_fmt(args).unwrap();
    });
    if LOG_ON_SCREEN.load(core::sync::atomic::Ordering::Relaxed) {
        SCREEN_LOGGER.try_with_locked(|writer| {
            writer.write_fmt(args).unwrap();
        });
    }
}

#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => {
        #[cfg(debug_assertions)]
        $crate::log::log(format_args!("[DEBUG] {}\n", format_args!($($arg)*)))
    };
}

#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {
        $crate::log::log(format_args!("[INFO ] {}\n", format_args!($($arg)*)))
    };
}

#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => {
        $crate::log::log(format_args!("[WARN ] {}\n", format_args!($($arg)*)))
    };
}

#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {
        $crate::log::log(format_args!("[ERROR] {}\n", format_args!($($arg)*)))
    };
}
