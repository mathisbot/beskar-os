//! This module contains the global logger instance used by the `log` crate.
//!
//! This logger is only intended to be used during the bootloader phase and will NOT be available
//! after the jump to the kernel.

use core::fmt::Write;
use spin::once::Once;

use crate::{
    screen::{self, Window},
    serdebug,
    utils::locks::McsLock,
};

mod writer;

/// The global logger instance used for the `log` crate.
pub static LOGGER: Once<LockedLogger> = Once::new();

pub struct LockedLogger {
    window_writer: McsLock<writer::WindowWriter>,
}

impl LockedLogger {
    #[must_use]
    #[inline]
    /// Create a new instance that logs to the given framebuffer.
    pub const fn new(window: Window) -> Self {
        let writer = writer::WindowWriter::new(window);

        Self {
            window_writer: McsLock::new(writer),
        }
    }
}

impl log::Log for LockedLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        if cfg!(debug_assertions) {
            true
        } else {
            metadata.level() <= log::Level::Info
        }
    }

    fn log(&self, record: &log::Record) {
        self.window_writer.with_locked(|writer| {
            writeln!(writer, "[{:5}] {}", record.level(), record.args()).unwrap();

            let screen = screen::get_screen();
            screen.present_window(writer.window());
        });
    }

    fn flush(&self) {}
}

pub fn init() {
    let window = screen::get_screen()
        .create_window(55, 25, 1000, 1000)
        .unwrap();
    serdebug!("Logger window created");

    let logger = LOGGER.call_once(|| LockedLogger::new(window));

    log::set_logger(logger).expect("Failed to set logger");
    log::set_max_level(if cfg!(debug_assertions) {
        log::LevelFilter::Trace
    } else {
        log::LevelFilter::Info
    });
}
