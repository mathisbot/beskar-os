use noto_sans_mono_bitmap::{
    FontWeight, RasterHeight, RasterizedChar, get_raster, get_raster_width,
};

use crate::screen::{self, pixel::Pixel};

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
pub struct ScreenWriter {
    screen_info: screen::Info,
    x: usize,
    y: usize,
}

impl ScreenWriter {
    #[must_use]
    #[inline]
    pub fn new() -> Self {
        Self {
            screen_info: screen::with_screen(|screen| screen.info()),
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

    fn clear_screen(&mut self, buffer: &mut [u32]) {
        self.x = BORDER_PADDING;
        self.y = BORDER_PADDING;
        buffer.fill(0);
    }

    /// Writes a single char to the framebuffer.
    ///
    /// Handles control characters (newline and carriage return).
    fn write_char(&mut self, buffer: &mut [u32], c: char) {
        match c {
            '\n' => self.newline(),
            '\r' => self.carriage_return(),
            c => {
                if self.x + CHAR_WIDTH + BORDER_PADDING >= self.screen_info.width {
                    self.newline();
                }
                if self.y + CHAR_HEIGHT.val() + LINE_SPACING + BORDER_PADDING
                    >= self.screen_info.height
                {
                    // TODO: Clear or scroll up ?
                    self.clear_screen(buffer);
                }

                let rasterized_char = get_raster_backed(c);

                for (v, row) in rasterized_char.raster().iter().enumerate() {
                    for (u, byte) in row.iter().enumerate() {
                        let pixel = Pixel::new(self.screen_info.pixel_format, *byte, *byte, *byte);
                        self.write_pixel(buffer, self.x + u, self.y + v, pixel.into());
                    }
                }
                self.x += rasterized_char.width() + LETTER_SPACING;
            }
        }
    }

    #[inline]
    fn write_pixel(&self, raw_framebuffer: &mut [u32], x: usize, y: usize, pixel: u32) {
        raw_framebuffer[y * self.screen_info.stride + x] = pixel;
    }
}

// TODO: Color support
impl core::fmt::Write for ScreenWriter {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        screen::with_screen(|screen| {
            let buffer = screen.buffer_mut();
            for c in s.chars() {
                self.write_char(buffer, c);
            }
        });
        Ok(())
    }
}
