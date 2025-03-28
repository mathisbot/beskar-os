use super::{Pixel, PixelComponents};
use crate::video::Info;

mod chars;
use chars::{CHAR_HEIGHT, CHAR_WIDTH, get_raster_backed};

pub const LINE_SPACING: usize = 2;
pub const LETTER_SPACING: usize = 0;
pub const BORDER_PADDING: usize = 3;

/// Allows logging text to a pixel-based framebuffer.
pub struct FramebufferWriter {
    info: Info,
    x: usize,
    y: usize,
    curr_color: PixelComponents,
}

impl FramebufferWriter {
    #[must_use]
    #[inline]
    pub fn new(info: Info) -> Self {
        Self {
            info,
            x: BORDER_PADDING,
            y: BORDER_PADDING,
            curr_color: Pixel::WHITE.components_by_format(info.pixel_format()),
        }
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
    pub const fn set_color(&mut self, color: PixelComponents) {
        self.curr_color = color;
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
                    self.clear_screen(buffer);
                }

                let rasterized_char = get_raster_backed(c);

                for (v, row) in rasterized_char.raster().iter().enumerate() {
                    for (u, byte) in row.iter().enumerate() {
                        let pixel_components = PixelComponents {
                            red: *byte,
                            green: *byte,
                            blue: *byte,
                        } * self.curr_color;
                        let pixel = Pixel::from_format(self.info.pixel_format, pixel_components);
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
