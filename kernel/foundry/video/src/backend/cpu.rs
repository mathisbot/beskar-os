//! CPU-based rendering backend using memcpy
use crate::backend::{RenderBackend, RenderError};
use beskar_core::video::{Info, PixelFormat, Point, Rect};

/// CPU-based rendering backend using memcpy
pub struct CpuBackend {
    dst: *mut u8,
    info: Info,
}

// SAFETY: Caller must ensure dst pointer is valid for the entire lifetime of this backend
unsafe impl Send for CpuBackend {}

impl CpuBackend {
    /// Create a new CPU-based rendering backend
    ///
    /// # Safety
    ///
    /// Callers must ensure that `dst` points to valid memory for the framebuffer,
    /// and that it remains valid for the lifetime of this backend.
    #[must_use]
    #[inline]
    pub const unsafe fn new(dst: *mut u8, info: Info) -> Self {
        Self { dst, info }
    }

    /// Convert a 32-bit color from one format to another
    fn convert_color(color: u32, from: PixelFormat, to: PixelFormat) -> u32 {
        if from == to {
            return color;
        }

        // Simplified conversion
        match (from, to) {
            (PixelFormat::Argb8888, PixelFormat::Xrgb8888) => color & 0x00FF_FFFF,
            (PixelFormat::Xrgb8888, PixelFormat::Argb8888) => color | 0xFF00_0000,
            _ => color,
        }
    }
}

impl RenderBackend for CpuBackend {
    unsafe fn bitblt(
        &mut self,
        src_pixels: *const u8,
        src_stride: u16,
        src_rect: Rect,
        dst_pos: Point,
    ) -> Result<(), RenderError> {
        let src_bpp = u32::from(self.info.bytes_per_pixel());

        if dst_pos.x >= self.info.width() || dst_pos.y >= self.info.height() {
            return Err(RenderError::OutOfBounds);
        }

        let actual_width = src_rect
            .width
            .min(self.info.width().saturating_sub(dst_pos.x));
        let actual_height = src_rect
            .height
            .min(self.info.height().saturating_sub(dst_pos.y));
        if actual_width == 0 || actual_height == 0 {
            return Ok(());
        }

        for row in 0..actual_height {
            let src_y = src_rect.top_left.y + row;
            let dst_y = dst_pos.y + row;

            let src_offset = src_y as usize * src_stride as usize
                + src_rect.top_left.x as usize * src_bpp as usize;

            let dst_offset_pixels = dst_y as usize * self.info.stride() as usize;
            let dst_offset =
                dst_offset_pixels * src_bpp as usize + dst_pos.x as usize * src_bpp as usize;

            // SAFETY: Caller must ensure pointers are valid for the entire operation
            let src_row = unsafe { src_pixels.byte_add(src_offset) };
            let dst_row = unsafe { self.dst.byte_add(dst_offset) };

            let row_bytes = (actual_width as usize) * (src_bpp as usize);

            // SAFETY: Caller guarantees valid pointers
            unsafe {
                core::ptr::copy_nonoverlapping(src_row, dst_row, row_bytes);
            }
        }

        Ok(())
    }

    unsafe fn fill(&mut self, rect: Rect, color: u32) -> Result<(), RenderError> {
        let bpp = self.info.bytes_per_pixel();
        let color = Self::convert_color(color, PixelFormat::Argb8888, self.info.pixel_format());

        if rect.top_left.x >= self.info.width() || rect.top_left.y >= self.info.height() {
            return Err(RenderError::OutOfBounds);
        }

        let actual_width = rect
            .width
            .min(self.info.width().saturating_sub(rect.top_left.x));
        let actual_height = rect
            .height
            .min(self.info.height().saturating_sub(rect.top_left.y));
        if actual_width == 0 || actual_height == 0 {
            return Ok(());
        }

        for row in 0..actual_height {
            let y = rect.top_left.y + row;
            let offset_pixels = y as usize * self.info.stride() as usize;
            let offset = offset_pixels * bpp as usize + rect.top_left.x as usize * bpp as usize;

            // SAFETY: Caller must ensure pointer is valid
            let dst_row = unsafe { self.dst.add(offset) };

            for col in 0..actual_width {
                let x_offset = col as usize * bpp as usize;
                // SAFETY: Caller must ensure pointer is valid
                let pixel_ptr = unsafe { dst_row.add(x_offset) };

                match bpp {
                    4 => {
                        // SAFETY: Caller guarantees valid aligned pointer for at least 4 bytes
                        #[expect(clippy::cast_ptr_alignment)]
                        unsafe {
                            *(pixel_ptr.cast::<u32>()) = color;
                        }
                    }
                    _ => return Err(RenderError::InvalidFormat),
                }
            }
        }

        Ok(())
    }
}
