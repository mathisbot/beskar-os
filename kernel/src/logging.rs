//! This module contains the global logger instance used by the `log` crate.
//!
//! This logger is only intended to be used during the bootloader phase and will NOT be available
//! after the jump to the kernel.

use core::fmt::Write;

use crate::{screen, serdebug};
use hyperdrive::locks::mcs::MUMcsLock;

mod writer;

/// The global logger instance used for the `log` crate.
static LOGGER: MUMcsLock<writer::WindowWriter> = MUMcsLock::uninit();

/// The backed logger instance used for the `log` crate.
pub static LOGGER_API: LockedLogger = LockedLogger;

/// An API that is backed by a static locked logger.
///
/// It is used to interface with the `log` crate.
pub struct LockedLogger;

impl log::Log for LockedLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        if cfg!(debug_assertions) {
            true
        } else {
            metadata.level() <= log::Level::Info
        }
    }

    fn log(&self, record: &log::Record) {
        LOGGER
            .try_with_locked(|writer| {
                let res = writeln!(writer, "[{:5}] {}", record.level(), record.args());

                let screen = screen::get_screen();
                screen.present_window(writer.window());

                res
            })
            .map(Result::unwrap);
    }

    fn flush(&self) {}
}

pub fn init() {
    let window = screen::get_screen()
        .create_window(55, 25, 1000, 1000)
        .unwrap();
    serdebug!("Logger window created");

    let window_writer = writer::WindowWriter::new(window);
    LOGGER.init(window_writer);

    log::set_logger(&LOGGER_API).expect("Failed to set logger");
    log::set_max_level(if cfg!(debug_assertions) {
        log::LevelFilter::Trace
    } else {
        log::LevelFilter::Info
    });
}
