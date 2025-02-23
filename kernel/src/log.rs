use crate::screen::with_screen;
use beskar_core::{
    arch::x86_64::port::serial::com::{ComNumber, SerialCom},
    video::writer::FramebufferWriter,
};
use core::{fmt::Write, sync::atomic::AtomicBool};
use hyperdrive::locks::mcs::{MUMcsLock, McsLock};

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
    LOG_ON_SCREEN.store(enable, core::sync::atomic::Ordering::Release);
}

pub fn log(args: core::fmt::Arguments) {
    SERIAL.with_locked(|serial| {
        serial.write_fmt(args).unwrap();
    });
    if LOG_ON_SCREEN.load(core::sync::atomic::Ordering::Acquire) {
        SCREEN_LOGGER.with_locked_if_init(|writer| {
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

/// Allows logging text to a pixel-based framebuffer.
pub struct ScreenWriter(FramebufferWriter);

impl Default for ScreenWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl ScreenWriter {
    #[must_use]
    #[inline]
    pub fn new() -> Self {
        let info = with_screen(|screen| screen.info());
        Self(FramebufferWriter::new(info))
    }
}

impl core::fmt::Write for ScreenWriter {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        with_screen(|screen| {
            self.0.write_str(screen.buffer_mut(), s);
        });
        Ok(())
    }
}
