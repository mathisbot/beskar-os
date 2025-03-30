use beskar_core::arch::x86_64::port::serial::com::{ComNumber, SerialCom};
use beskar_core::video::writer::FramebufferWriter;
use beskar_core::video::{Info, PixelComponents};
use core::fmt::Write;
use hyperdrive::locks::mcs::MUMcsLock;

static SERIAL: MUMcsLock<SerialCom> = MUMcsLock::uninit();

static SCREEN_LOGGER: MUMcsLock<ScreenWriter> = MUMcsLock::uninit();

pub fn init_serial() {
    let mut serial = SerialCom::new(ComNumber::Com1);
    if serial.init().is_ok() {
        SERIAL.init(serial);
    }
}

pub fn init_screen() {
    let info = crate::video::with_physical_framebuffer(|screen| screen.info());
    let screen = ScreenWriter::new(info);
    SCREEN_LOGGER.init(screen);
}

pub fn log(severity: Severity, args: core::fmt::Arguments) {
    SERIAL.with_locked_if_init(|serial| {
        serial.write_char('[').unwrap();
        serial.write_str(severity.as_str()).unwrap();
        serial.write_char(']').unwrap();
        serial.write_char(' ').unwrap();
        serial.write_fmt(args).unwrap();
        serial.write_char('\n').unwrap();
    });
    SCREEN_LOGGER.with_locked_if_init(|writer| {
        writer.write_char('[').unwrap();
        writer.set_color(severity.color());
        writer.write_str(severity.as_str()).unwrap();
        writer.set_color(PixelComponents::WHITE);
        writer.write_char(']').unwrap();
        writer.write_char(' ').unwrap();
        writer.write_fmt(args).unwrap();
        writer.write_char('\n').unwrap();
    });
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
            Self::Warn => PixelComponents::new(255, 120, 0),
            Self::Error => PixelComponents::RED,
        }
    }
}

#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => {
        #[cfg(debug_assertions)]
        $crate::log::log($crate::log::Severity::Debug, format_args!($($arg)*));
    };
}

#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {
        $crate::log::log($crate::log::Severity::Info, format_args!($($arg)*));
    };
}

#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => {
        $crate::log::log($crate::log::Severity::Warn, format_args!($($arg)*));
    };
}

#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {
        $crate::log::log($crate::log::Severity::Error, format_args!($($arg)*));
    };
}

/// Allows logging text to a pixel-based framebuffer.
pub struct ScreenWriter(FramebufferWriter);

impl ScreenWriter {
    #[must_use]
    #[inline]
    pub fn new(info: Info) -> Self {
        Self(FramebufferWriter::new(info))
    }

    #[inline]
    pub const fn set_color(&mut self, color: PixelComponents) {
        self.0.set_color(color);
    }
}

impl core::fmt::Write for ScreenWriter {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        crate::video::with_physical_framebuffer(|screen| {
            self.0.write_str(screen.buffer_mut(), s);
        });
        Ok(())
    }
}
