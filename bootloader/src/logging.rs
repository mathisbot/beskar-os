//! This module contains the global logger instance used by the `log` crate.
//!
//! This logger is only intended to be used during the bootloader phase and will NOT be available
//! after the jump to the kernel.

use crate::framebuffer::FrameBufferWriter;
use crate::FrameBufferInfo;
use core::fmt::Write;
use hyperdrive::locks::mcs::MUMcsLock;

/// The global logger instance used for the `log` crate.
static LOGGER: MUMcsLock<FrameBufferWriter> = MUMcsLock::uninit();

/// The static API for the logger.
pub static LOGGER_API: LockedLogger = LockedLogger;

/// An API that is backed by a static locked logger.
///
/// It is used to interface with the `log` crate.
pub struct LockedLogger;

pub fn init(framebuffer: &'static mut [u8], info: FrameBufferInfo) -> &'static LockedLogger {
    let logger = FrameBufferWriter::new(framebuffer, info);
    LOGGER.init(logger);
    &LOGGER_API
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
        if cfg!(debug_assertions) {
            LOGGER
                .with_locked(|fb| {
                    writeln!(
                        fb,
                        "[{:5}] {}:{}: {}",
                        record.level(),
                        record.file().unwrap_or("unknown"),
                        record.line().unwrap_or(0),
                        record.args()
                    )
                })
                .unwrap();
        } else {
            LOGGER
                .with_locked(|fb| writeln!(fb, "[{:5}] {}", record.level(), record.args()))
                .unwrap();
        }
    }

    fn flush(&self) {}
}
