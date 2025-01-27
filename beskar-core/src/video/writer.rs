use super::Pixel;
use crate::video::Info;

mod chars;
use chars::{
    BORDER_PADDING, CHAR_HEIGHT, CHAR_WIDTH, LETTER_SPACING, LINE_SPACING, get_raster_backed,
};

/// Allows logging text to a pixel-based framebuffer.
pub struct FramebufferWriter {
    info: Info,
    x: usize,
    y: usize,
}

impl FramebufferWriter {
    #[must_use]
    #[inline]
    pub const fn new(info: Info) -> Self {
        Self { info, x: 0, y: 0 }
    }

    #[inline]
    const fn newline(&mut self) {
        self.y += CHAR_HEIGHT + LINE_SPACING;
        self.carriage_return();
    }

    #[inline]
    const fn carriage_return(&mut self) {
        self.x = BORDER_PADDING;
    }

    #[inline]
    fn clear_screen(&mut self, buffer: &mut [Pixel]) {
        self.x = BORDER_PADDING;
        self.y = BORDER_PADDING;
        buffer.fill(Pixel::BLACK);
    }

    #[inline]
    /// Writes a string to the framebuffer.
    pub fn write_str(&mut self, buffer: &mut [Pixel], s: &str) {
        for c in s.chars() {
            self.write_char(buffer, c);
        }
    }

    /// Writes a single char to the framebuffer.
    ///
    /// Handles control characters (newline and carriage return).
    pub fn write_char(&mut self, buffer: &mut [Pixel], c: char) {
        match c {
            '\n' => self.newline(),
            '\r' => self.carriage_return(),
            c => {
                if self.x + CHAR_WIDTH + BORDER_PADDING >= self.info.width() {
                    self.newline();
                }
                if self.y + CHAR_HEIGHT + LINE_SPACING + BORDER_PADDING >= self.info.height() {
                    // TODO: Clear or scroll up ?
                    self.clear_screen(buffer);
                }

                let rasterized_char = get_raster_backed(c);

                for (v, row) in rasterized_char.raster().iter().enumerate() {
                    for (u, byte) in row.iter().enumerate() {
                        let pixel = Pixel::from_format(self.info.pixel_format, *byte, *byte, *byte);
                        self.write_pixel(buffer, self.x + u, self.y + v, pixel);
                    }
                }
                self.x += rasterized_char.width() + LETTER_SPACING;
            }
        }
    }

    #[inline]
    fn write_pixel(&self, raw_framebuffer: &mut [Pixel], x: usize, y: usize, pixel: Pixel) {
        raw_framebuffer[y * self.info.stride + x] = pixel;
    }
}
