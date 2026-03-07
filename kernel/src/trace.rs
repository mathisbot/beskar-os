//! Kernel logging subsystem
//!
//! Provides logging to:
//! - Serial port (debug builds only, immediate output)
//! - Screen (via compositor surface, damage-tracked)
//!
//! # Architecture
//!
//! The screen logger is a CLIENT of the compositor, not a driver.
//! - ScreenWriter writes pixels to its surface buffer
//! - Marks the surface dirty after each write
//! - Does NOT trigger rendering directly
//! - Compositor decides when to render based on policy
//!
//! This decoupling is critical for:
//! - Performance: batch multiple log messages into one render
//! - Safety: avoid rendering in interrupt context
//! - Flexibility: logging works regardless of render policy

use beskar_core::{
    arch::paging::M4KiB,
    video::{
        Pixel, PixelComponents, Rect, SurfaceId,
        writer::{CHAR_HEIGHT, FramebufferWriter, LINE_SPACING},
    },
};
use beskar_hal::paging::page_table::Flags;
use beskar_hal::port::serial::com::{ComNumber, SerialCom};
use core::{
    fmt::Write,
    sync::atomic::{AtomicBool, Ordering},
};
use hyperdrive::locks::mcs::MUMcsLock;

static SERIAL: MUMcsLock<SerialCom> = MUMcsLock::uninit();

static LOG_ON_SCREEN: AtomicBool = AtomicBool::new(true);
static SCREEN_LOGGER: MUMcsLock<ScreenWriter> = MUMcsLock::uninit();

/// Initialize the serial logger.
///
/// This function should be called at the very beginning of the kernel.
pub fn init_serial() {
    if cfg!(debug_assertions) {
        let mut serial = SerialCom::new(ComNumber::Com1);
        if serial.init().is_ok() {
            SERIAL.init(serial);
        }
    }
}

/// Initialize the screen logger.
///
/// This function should be called after the video compositor has been initialized.
pub fn init_screen() {
    let info = crate::video::with_compositor(|c| c.config().info()).unwrap();
    let width = info.width();
    let height = info.height();

    let screen = ScreenWriter::new(width, height);
    SCREEN_LOGGER.init(screen);
}

/// Enable or disable screen logging
#[inline]
pub fn set_screen_logging(enable: bool) {
    LOG_ON_SCREEN.store(enable, Ordering::Release);
}

/// Core logging function
///
/// Writes to serial (immediate) and screen (damage-tracked).
/// Screen writes do NOT trigger rendering - that's the compositor's decision.
pub fn log(severity: Severity, args: core::fmt::Arguments) {
    // Serial: immediate output for debugging
    #[cfg(debug_assertions)]
    SERIAL.with_locked_if_init(|serial| {
        serial.write_char('[').unwrap();
        serial.write_str(severity.as_str()).unwrap();
        serial.write_char(']').unwrap();
        serial.write_char(' ').unwrap();
        serial.write_fmt(args).unwrap();
    });

    // Screen: buffered output via compositor
    if LOG_ON_SCREEN.load(Ordering::Acquire) {
        let sid = SCREEN_LOGGER.with_locked_if_init(|writer| {
            writer.write_char('[').unwrap();
            writer.set_color(severity.color());
            writer.write_str(severity.as_str()).unwrap();
            writer.set_color(PixelComponents::WHITE);
            writer.write_char(']').unwrap();
            writer.write_char(' ').unwrap();
            writer.write_fmt(args).unwrap();
            writer.surface_id
        });
        if let Some(sid) = sid {
            crate::video::with_compositor(|c| c.render_surface_dirty(sid));
        }
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
        $crate::trace::log($crate::trace::Severity::Debug, format_args!("\n"));
    };
    ($fmt:expr) => {
        $crate::trace::log($crate::trace::Severity::Debug, format_args!(concat!($fmt, "\n")));
    };
    ($fmt:expr, $($arg:tt)*) => {
        $crate::trace::log($crate::trace::Severity::Debug, format_args!(concat!($fmt, "\n"), $($arg)*));
    };
}

#[macro_export]
macro_rules! info {
    () => {
        $crate::trace::log($crate::trace::Severity::Info, format_args!("\n"));
    };
    ($fmt:expr) => {
        $crate::trace::log($crate::trace::Severity::Info, format_args!(concat!($fmt, "\n")));
    };
    ($fmt:expr, $($arg:tt)*) => {
        $crate::trace::log($crate::trace::Severity::Info, format_args!(concat!($fmt, "\n"), $($arg)*));
    };
}

#[macro_export]
macro_rules! warn {
    () => {
        $crate::trace::log($crate::trace::Severity::Warn, format_args!("\n"));
    };
    ($fmt:expr) => {
        $crate::trace::log($crate::trace::Severity::Warn, format_args!(concat!($fmt, "\n")));
    };
    ($fmt:expr, $($arg:tt)*) => {
        $crate::trace::log($crate::trace::Severity::Warn, format_args!(concat!($fmt, "\n"), $($arg)*));
    };
}

#[macro_export]
macro_rules! error {
    () => {
        $crate::trace::log($crate::trace::Severity::Error, format_args!("\n"));
    };
    ($fmt:expr) => {
        $crate::trace::log($crate::trace::Severity::Error, format_args!(concat!($fmt, "\n")));
    };
    ($fmt:expr, $($arg:tt)*) => {
        $crate::trace::log($crate::trace::Severity::Error, format_args!(concat!($fmt, "\n"), $($arg)*));
    };
}

/// Screen writer for text logging
pub struct ScreenWriter<'a> {
    surface_id: SurfaceId,
    writer: FramebufferWriter,
    buffer: &'a mut [Pixel],
}

impl ScreenWriter<'_> {
    #[must_use]
    #[inline]
    pub fn new(width: u16, height: u16) -> Self {
        let compositor_info = crate::video::with_compositor(|c| c.config().info()).unwrap();

        let buffer_size =
            width as usize * height as usize * compositor_info.bytes_per_pixel() as usize;

        // Allocate buffer in kernel address space
        let page_range = crate::mem::address_space::get_kernel_address_space()
            .alloc_map_zeroed::<M4KiB>(buffer_size, Flags::PRESENT | Flags::WRITABLE)
            .unwrap();
        let buffer_start = page_range.start().start_address();

        let buffer = unsafe {
            core::slice::from_raw_parts_mut(
                buffer_start.as_mut_ptr::<Pixel>(),
                buffer_size / core::mem::size_of::<Pixel>(),
            )
        };

        crate::info!(
            "Allocated screen logger buffer at {:#x} ({} bytes)",
            buffer_start.as_u64(),
            buffer_size
        );

        // Create surface in compositor
        let surface_id = crate::video::with_compositor(|c| unsafe {
            c.create_surface_with_buffer(0, 0, width, height, buffer_start.as_mut_ptr())
        })
        .unwrap();

        crate::info!("Created screen logger surface with ID {:?}", surface_id);

        let local_info =
            beskar_core::video::Info::new(width, height, compositor_info.pixel_format(), width);
        let writer = FramebufferWriter::new(local_info);

        Self {
            surface_id,
            writer,
            buffer,
        }
    }

    #[inline]
    pub const fn set_color(&mut self, color: PixelComponents) {
        self.writer.set_color(color);
    }

    /// Mark a specific region of the surface dirty after writing
    fn mark_dirty_region(&self, rect: Rect) {
        let _ = crate::video::with_compositor(|c| c.mark_surface_dirty(self.surface_id, rect));
    }

    /// Mark the entire surface dirty (e.g. after a full clear)
    fn mark_all_dirty(&self) {
        let _ = crate::video::with_compositor(|c| c.mark_surface_all_dirty(self.surface_id));
    }
}

impl core::fmt::Write for ScreenWriter<'_> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let y_before = self.writer.y();
        self.writer.write_str(self.buffer, s);
        let y_after = self.writer.y();

        // If a screen clear happened during writing (cursor wrapped back to top),
        // the whole surface must be considered dirty.
        if y_after < y_before {
            self.mark_all_dirty();
        } else {
            // Cover all rows that were actually drawn into.
            let dirty_height = (y_after - y_before) + CHAR_HEIGHT + LINE_SPACING;
            let rect = Rect::new(0, y_before, self.writer.info().width(), dirty_height);
            self.mark_dirty_region(rect);
        }

        Ok(())
    }
}
