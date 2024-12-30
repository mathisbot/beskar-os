use noto_sans_mono_bitmap::{
    get_raster, get_raster_width, FontWeight, RasterHeight, RasterizedChar,
};

use crate::screen::{self, pixel::PixelFormat, Window};
use hyperdrive::locks::mcs::McsNode;

const LINE_SPACING: usize = 2;
const LETTER_SPACING: usize = 0;
const BORDER_PADDING: usize = 3;

const CHAR_HEIGHT: RasterHeight = RasterHeight::Size20;
const CHAR_WIDTH: usize = get_raster_width(FontWeight::Regular, CHAR_HEIGHT);
const BACKUP_CHAR: char = '?';

#[must_use]
/// Returns the raster of the given char,
/// backing up to a default char if the given char is not available.
fn get_raster_backed(c: char) -> RasterizedChar {
    get_raster(c, FontWeight::Regular, CHAR_HEIGHT)
        .unwrap_or_else(|| get_raster(BACKUP_CHAR, FontWeight::Regular, CHAR_HEIGHT).unwrap())
}

/// Allows logging text to a pixel-based framebuffer.
pub struct WindowWriter {
    window: Window,
    x: usize,
    y: usize,
}

impl Drop for WindowWriter {
    fn drop(&mut self) {
        let screen = screen::get_screen();
        unsafe { screen.destroy_window(&self.window) };
    }
}

impl WindowWriter {
    #[must_use]
    #[inline]
    pub const fn new(window: Window) -> Self {
        Self {
            window,
            x: BORDER_PADDING,
            y: BORDER_PADDING,
        }
    }

    #[inline]
    const fn newline(&mut self) {
        self.y += CHAR_HEIGHT.val() + LINE_SPACING;
        self.carriage_return();
    }

    #[inline]
    const fn carriage_return(&mut self) {
        self.x = BORDER_PADDING;
    }

    pub fn clear(&mut self) {
        self.x = BORDER_PADDING;
        self.y = BORDER_PADDING;
        let mut node = McsNode::new();
        self.window.buffer_mut(&mut node).fill(0);
    }

    /// Writes a single char to the framebuffer.
    ///
    /// Handles control characters (newline and carriage return).
    fn write_char(&mut self, c: char) {
        match c {
            '\n' => self.newline(),
            '\r' => self.carriage_return(),
            c => {
                if self.x + CHAR_WIDTH + BORDER_PADDING >= self.window.width() {
                    self.newline();
                }
                if self.y + CHAR_HEIGHT.val() + LINE_SPACING + BORDER_PADDING
                    >= self.window.height()
                {
                    // TODO: Clear or scroll up ?
                    // let offset = self.info.stride * (CHAR_HEIGHT.val() + LINE_SPACING) * self.info.bytes_per_pixel;
                    // unsafe {
                    //     core::ptr::copy(
                    //         self.raw_framebuffer
                    //             .as_ptr()
                    //             .add(offset),
                    //         self.raw_framebuffer.as_mut_ptr(),
                    //         self.info.size - offset,
                    //     )
                    // };
                    // self.carriage_return();
                    // self.y -= CHAR_HEIGHT.val() + LINE_SPACING;
                    self.clear();
                }

                let rasterized_char = get_raster_backed(c);

                let mut node = McsNode::new();
                let mut raw_framebuffer = self.window.buffer_mut(&mut node);

                for (v, row) in rasterized_char.raster().iter().enumerate() {
                    for (u, byte) in row.iter().enumerate() {
                        // FIXME: Better skip black pixels
                        if *byte == 0 {
                            continue;
                        }
                        self.write_pixel(&mut raw_framebuffer, self.x + u, self.y + v, *byte);
                    }
                }
                self.x += rasterized_char.width() + LETTER_SPACING;
            }
        }
    }

    fn write_pixel(&self, raw_framebuffer: &mut [u8], x: usize, y: usize, intensity: u8) {
        let color = match self.window.pixel_format() {
            PixelFormat::Rgb | PixelFormat::Bgr => [intensity, intensity, intensity, 0],
            // PixelFormat::Bitmask(bitmask) => {
            //     // Intensity is thown away
            //     let is_on = intensity > u8::MAX / 2;

            //     let mut color = 0_u32;
            //     color |= if is_on { bitmask.red } else { 0 };
            //     color |= if is_on { bitmask.green } else { 0 };
            //     color |= if is_on { bitmask.blue } else { 0 };

            //     color.to_ne_bytes()
            // }
        };

        let bytes_per_pixel = self.window.bytes_per_pixel();
        let byte_offset = (y * self.window.width() + x) * bytes_per_pixel;

        raw_framebuffer[byte_offset..(byte_offset + bytes_per_pixel)]
            .copy_from_slice(&color[..bytes_per_pixel]);
    }

    pub const fn window(&self) -> &Window {
        &self.window
    }
}

impl core::fmt::Write for WindowWriter {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for c in s.chars() {
            self.write_char(c);
        }
        Ok(())
    }
}
