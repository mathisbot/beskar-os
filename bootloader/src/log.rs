use beskar_core::arch::x86_64::port::serial::com::{ComNumber, SerialCom};
use beskar_core::video::Info;
use beskar_core::video::writer::FramebufferWriter;
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

pub fn log(args: core::fmt::Arguments) {
    SERIAL.with_locked_if_init(|serial| {
        serial.write_fmt(args).unwrap();
    });
    SCREEN_LOGGER.with_locked_if_init(|writer| {
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

/// Allows logging text to a pixel-based framebuffer.
pub struct ScreenWriter(FramebufferWriter);

impl ScreenWriter {
    #[must_use]
    #[inline]
    pub const fn new(info: Info) -> Self {
        Self(FramebufferWriter::new(info))
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
