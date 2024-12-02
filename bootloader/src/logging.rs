//! This module contains the global logger instance used by the `log` crate.
//!
//! This logger is only intended to be used during the bootloader phase and will NOT be available
//! after the jump to the kernel.

use crate::framebuffer::FrameBufferWriter;
use crate::FrameBufferInfo;
use core::fmt::Write;
use spin::{mutex::SpinMutex, once::Once};

/// The global logger instance used for the `log` crate.
pub static LOGGER: Once<LockedLogger> = Once::new();

/// A logger instance protected by a spinlock.
pub struct LockedLogger {
    framebuffer: SpinMutex<FrameBufferWriter>,
}

impl LockedLogger {
    #[must_use]
    #[inline]
    /// Create a new instance that logs to the given framebuffer.
    pub fn new(framebuffer: &'static mut [u8], info: FrameBufferInfo) -> Self {
        let framebuffer = SpinMutex::new(FrameBufferWriter::new(framebuffer, info));

        Self { framebuffer }
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
        if cfg!(debug_assertions) {
            writeln!(
                self.framebuffer.lock(),
                "[{:5}] {}:{}: {}",
                record.level(),
                record.file().unwrap_or("unknown"),
                record.line().unwrap_or(0),
                record.args()
            )
            .unwrap();
        } else {
            writeln!(
                self.framebuffer.lock(),
                "[{:5}] {}",
                record.level(),
                record.args()
            )
            .unwrap();
        }
    }

    fn flush(&self) {}
}
