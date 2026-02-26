//! Screen surface management using the compositor Surface API
//!
//! This module manages two separate compositor surfaces:
//! - UI surface: Static UI elements (header, logo, borders)
//! - TTY surface: Dynamic terminal content (rendered on top)

use beskar_core::video::{Info, Pixel};
use beskar_lib::{error::IoResult, surface::Surface};
use hyperdrive::once::Once;

static SCREEN_INFO: Once<Info> = Once::uninit();
static UI_SURFACE: Once<OwnedSurface> = Once::uninit();
static TTY_SURFACE: Once<OwnedSurface> = Once::uninit();

/// Initialize UI surface with the full screen size and present it to the compositor
///
/// # Panics
///
/// Panics if called more than once or if surface creation fails.
pub fn init_ui_surface() {
    let info = screen_info();
    let ui_surface = OwnedSurface::new(info.width(), info.height(), 0, 0);
    ui_surface
        .surface
        .present_all()
        .expect("Failed to present UI surface");
    UI_SURFACE.call_once(|| ui_surface);
}

/// Initialize the TTY surface based on the UI layout
///
/// # Panics
///
/// Panics if called more than once or if surface creation fails.
pub fn init_tty_surface(x: u16, y: u16, width: u16, height: u16) {
    let tty_surface = OwnedSurface::new(width, height, x, y);
    tty_surface
        .surface
        .present_all()
        .expect("Failed to present TTY surface");
    TTY_SURFACE.call_once(|| tty_surface);
}

struct OwnedSurface {
    surface: Surface,
    buffer: &'static mut [Pixel],
    info: Info,
}

impl OwnedSurface {
    #[must_use]
    /// Create a new owned surface with the given parameters
    ///
    /// # Safety
    ///
    /// The caller must ensure that the buffer pointer is valid and has the correct size.
    pub fn new(width: u16, height: u16, x: u16, y: u16) -> Self {
        let info = beskar_lib::surface::query_screen_info().expect("Failed to query screen info");
        let buffer = {
            let pixel_count = u64::from(width) * u64::from(height);
            let size = pixel_count * core::mem::size_of::<Pixel>() as u64;
            let data =
                beskar_lib::mem::mmap(size, None, beskar_lib::mem::MemoryProtection::ReadWrite)
                    .unwrap()
                    .as_ptr();
            unsafe {
                core::slice::from_raw_parts_mut(data.cast(), usize::try_from(pixel_count).unwrap())
            }
        };
        buffer.fill(Pixel::BLACK);

        // SAFETY: buffer is valid and has the correct size
        let surface = unsafe { Surface::create(width, height, x, y, buffer.as_mut_ptr()) }
            .expect("Failed to create compositor surface");

        let info = Info::new(width, height, info.pixel_format(), width);
        Self {
            surface,
            buffer,
            info,
        }
    }
}

/// Returns the screen info
fn screen_info() -> &'static Info {
    SCREEN_INFO.call_once(|| {
        beskar_lib::surface::query_screen_info().expect("Failed to query screen info")
    });
    SCREEN_INFO.get().expect("Screen info not initialized")
}

/// Returns the UI surface info
///
/// # Panics
///
/// Panics if the UI surface is not initialized.
#[must_use]
pub fn ui_surface_info() -> &'static Info {
    &UI_SURFACE.get().expect("UI surface not initialized").info
}

/// Returns the TTY surface info
///
/// # Panics
///
/// Panics if the TTY surface is not initialized.
#[must_use]
pub fn tty_surface_info() -> &'static Info {
    &TTY_SURFACE.get().expect("TTY surface not initialized").info
}

/// Access the UI surface with a closure.
///
/// # Panics
///
/// Panics if the UI surface is not initialized.
pub fn with_ui_surface<R, F: FnOnce(&mut FrameBuffer<'_>) -> R>(f: F) -> R {
    let ctx = UI_SURFACE.get().expect("UI surface not initialized");
    // SAFETY: single-threaded; FrameBuffer cannot escape the closure.
    let buffer = unsafe {
        core::slice::from_raw_parts_mut(ctx.buffer.as_ptr().cast_mut(), ctx.buffer.len())
    };
    f(&mut FrameBuffer {
        surface: &ctx.surface,
        buffer,
        info: ctx.info,
    })
}

/// Access the TTY surface with a closure.
///
/// # Panics
///
/// Panics if the TTY surface is not initialized.
pub fn with_tty_surface<R, F: FnOnce(&mut FrameBuffer<'_>) -> R>(f: F) -> R {
    let ctx = TTY_SURFACE.get().expect("TTY surface not initialized");
    // SAFETY: single-threaded; FrameBuffer cannot escape the closure.
    let buffer = unsafe {
        core::slice::from_raw_parts_mut(ctx.buffer.as_ptr().cast_mut(), ctx.buffer.len())
    };
    f(&mut FrameBuffer {
        surface: &ctx.surface,
        buffer,
        info: ctx.info,
    })
}

const fn surface_error_to_io(_: beskar_lib::error::SurfaceError) -> beskar_lib::error::IoError {
    beskar_lib::error::IoError::new(beskar_lib::error::IoErrorKind::Other)
}

/// A short-lived handle to a compositor surface's pixel buffer.
///
/// Obtained exclusively via [`with_ui_surface`] or [`with_tty_surface`].
pub struct FrameBuffer<'a> {
    surface: &'a Surface,
    buffer: &'a mut [Pixel],
    info: Info,
}

impl FrameBuffer<'_> {
    /// Surface geometry and pixel format.
    #[must_use]
    #[inline]
    pub const fn info(&self) -> &Info {
        &self.info
    }

    /// Direct mutable access to the pixel buffer.
    ///
    /// Drop the returned slice before calling any `flush_*` method.
    #[must_use]
    #[inline]
    pub const fn pixels_mut(&mut self) -> &mut [Pixel] {
        self.buffer
    }

    /// Present a pixel-aligned rectangle to the compositor.
    ///
    /// # Errors
    ///
    /// Returns an `IoError` if the underlying syscall fails.
    pub fn flush_region(&self, x: u16, y: u16, width: u16, height: u16) -> IoResult<()> {
        self.surface
            .present_region(x, y, width, height)
            .map_err(surface_error_to_io)
    }

    /// Present the entire surface to the compositor.
    ///
    /// # Errors
    ///
    /// Returns an `IoError` if the underlying syscall fails.
    #[inline]
    pub fn flush_all(&self) -> IoResult<()> {
        self.surface.present_all().map_err(surface_error_to_io)
    }
}
