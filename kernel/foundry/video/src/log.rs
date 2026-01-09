use crate::screen::with_screen;
use beskar_core::video::{PixelComponents, writer::FramebufferWriter};
#[cfg(debug_assertions)]
use beskar_hal::port::serial::com::{ComNumber, SerialCom};
use core::{
    fmt::Write,
    sync::atomic::{AtomicBool, Ordering},
};
use hyperdrive::locks::mcs::MUMcsLock;

#[cfg(debug_assertions)]
static SERIAL: MUMcsLock<SerialCom> = MUMcsLock::uninit();

static LOG_ON_SCREEN: AtomicBool = AtomicBool::new(true);
static SCREEN_LOGGER: MUMcsLock<ScreenWriter> = MUMcsLock::uninit();

/// Initialize the serial logger.
///
/// This function should be called at the very beginning of the kernel.
pub fn init_serial() {
    #[cfg(debug_assertions)]
    {
        let mut serial = SerialCom::new(ComNumber::Com1);
        if serial.init().is_ok() {
            SERIAL.init(serial);
        }
    }
}

/// Initialize the screen logger.
///
/// This function should be called after the screen has been initialized.
pub fn init_screen() {
    let screen = ScreenWriter::new();
    SCREEN_LOGGER.init(screen);
}

#[inline]
pub fn set_screen_logging(enable: bool) {
    LOG_ON_SCREEN.store(enable, Ordering::Release);
}

pub fn log(severity: Severity, args: core::fmt::Arguments) {
    #[cfg(debug_assertions)]
    SERIAL.with_locked_if_init(|serial| {
        serial.write_char('[').unwrap();
        serial.write_str(severity.as_str()).unwrap();
        serial.write_char(']').unwrap();
        serial.write_char(' ').unwrap();
        serial.write_fmt(args).unwrap();
    });
    if LOG_ON_SCREEN.load(Ordering::Acquire) {
        SCREEN_LOGGER.with_locked_if_init(|writer| {
            writer.write_char('[').unwrap();
            writer.set_color(severity.color());
            writer.write_str(severity.as_str()).unwrap();
            writer.set_color(PixelComponents::WHITE);
            writer.write_char(']').unwrap();
            writer.write_char(' ').unwrap();
            writer.write_fmt(args).unwrap();
        });
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Debug,
    Info,
    Warn,
    Error,
}

impl Severity {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Debug => "DEBUG",
            Self::Info => "INFO ",
            Self::Warn => "WARN ",
            Self::Error => "ERROR",
        }
    }

    #[must_use]
    pub const fn color(self) -> PixelComponents {
        match self {
            Self::Debug => PixelComponents::BLUE,
            Self::Info => PixelComponents::GREEN,
            Self::Warn => PixelComponents::ORANGE,
            Self::Error => PixelComponents::RED,
        }
    }
}

#[macro_export]
macro_rules! debug {
    () => {
        $crate::log::log($crate::log::Severity::Debug, format_args!("\n"));
    };
    ($fmt:expr) => {
        $crate::log::log($crate::log::Severity::Debug, format_args!(concat!($fmt, "\n")));
    };
    ($fmt:expr, $($arg:tt)*) => {
        $crate::log::log($crate::log::Severity::Debug, format_args!(concat!($fmt, "\n"), $($arg)*));
    };
}

#[macro_export]
macro_rules! info {
    () => {
        $crate::log::log($crate::log::Severity::Info, format_args!("\n"));
    };
    ($fmt:expr) => {
        $crate::log::log($crate::log::Severity::Info, format_args!(concat!($fmt, "\n")));
    };
    ($fmt:expr, $($arg:tt)*) => {
        $crate::log::log($crate::log::Severity::Info, format_args!(concat!($fmt, "\n"), $($arg)*));
    };
}

#[macro_export]
macro_rules! warn {
    () => {
        $crate::log::log($crate::log::Severity::Warn, format_args!("\n"));
    };
    ($fmt:expr) => {
        $crate::log::log($crate::log::Severity::Warn, format_args!(concat!($fmt, "\n")));
    };
    ($fmt:expr, $($arg:tt)*) => {
        $crate::log::log($crate::log::Severity::Warn, format_args!(concat!($fmt, "\n"), $($arg)*));
    };
}

#[macro_export]
macro_rules! error {
    () => {
        $crate::log::log($crate::log::Severity::Error, format_args!("\n"));
    };
    ($fmt:expr) => {
        $crate::log::log($crate::log::Severity::Error, format_args!(concat!($fmt, "\n")));
    };
    ($fmt:expr, $($arg:tt)*) => {
        $crate::log::log($crate::log::Severity::Error, format_args!(concat!($fmt, "\n"), $($arg)*));
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

    #[inline]
    pub const fn set_color(&mut self, color: PixelComponents) {
        self.0.set_color(color);
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

pub(crate) fn with_fb_writer<R, F: FnOnce(&mut FramebufferWriter) -> R>(f: F) -> Option<R> {
    SCREEN_LOGGER.with_locked_if_init(|writer| f(&mut writer.0))
}
