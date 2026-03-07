use beskar_core::video::{Info, Pixel};
use beskar_lib::{error::SurfaceError, surface::Surface};

/// Manages a compositor surface with its backing pixel buffer.
pub struct Screen {
    surface: Surface,
    buffer: &'static mut [Pixel],
    info: Info,
}

impl Screen {
    /// Allocate a new surface at the given screen position and dimensions.
    ///
    /// # Panics
    ///
    /// Panics if memory allocation or surface creation fails.
    #[must_use]
    pub fn new(width: u16, height: u16, x: u16, y: u16) -> Self {
        let screen_info =
            beskar_lib::surface::query_screen_info().expect("Failed to query screen info");

        let pixel_count = u64::from(width) * u64::from(height);
        let byte_size = pixel_count * core::mem::size_of::<Pixel>() as u64;
        let data = beskar_lib::mem::mmap(
            byte_size,
            None,
            beskar_lib::mem::MemoryProtection::ReadWrite,
        )
        .expect("Failed to mmap surface buffer");

        let buffer = unsafe {
            let count = usize::try_from(pixel_count).unwrap();
            // Buffer is freshly mmap'd and Pixel is repr(transparent) over u32.
            // The kernel guarantees page-aligned memory, satisfying Pixel alignment (4).
            #[expect(clippy::cast_ptr_alignment)]
            let ptr = data.as_ptr().cast::<Pixel>();
            let slice = core::slice::from_raw_parts_mut(ptr, count);
            slice.fill(Pixel::BLACK);
            slice
        };

        let surface = unsafe { Surface::create(width, height, x, y, buffer.as_mut_ptr()) }
            .expect("Failed to create compositor surface");

        let info = Info::new(width, height, screen_info.pixel_format(), width);
        Self {
            surface,
            buffer,
            info,
        }
    }

    /// Full-screen surface covering the entire display.
    ///
    /// # Panics
    ///
    /// Panics if screen info query, memory allocation, or surface creation fails.
    #[must_use]
    pub fn fullscreen() -> Self {
        let info = beskar_lib::surface::query_screen_info().expect("Failed to query screen info");
        Self::new(info.width(), info.height(), 0, 0)
    }

    /// Framebuffer layout.
    #[must_use]
    #[inline]
    pub const fn info(&self) -> &Info {
        &self.info
    }

    /// Mutable access to the backing pixel buffer.
    #[must_use]
    #[inline]
    pub const fn pixels_mut(&mut self) -> &mut [Pixel] {
        self.buffer
    }

    /// Present the entire surface to the compositor.
    ///
    /// # Errors
    ///
    /// Returns `SurfaceError` if the compositor rejects the presentation.
    pub fn present_all(&self) -> Result<(), SurfaceError> {
        self.surface.present_all()
    }

    /// Present a rectangular region.
    ///
    /// # Errors
    ///
    /// Returns `SurfaceError` if the compositor rejects the presentation.
    pub fn present_region(
        &self,
        x: u16,
        y: u16,
        width: u16,
        height: u16,
    ) -> Result<(), SurfaceError> {
        self.surface.present_region(x, y, width, height)
    }
}
