#![no_std]
#![forbid(unsafe_op_in_unsafe_fn)]
#![warn(clippy::pedantic, clippy::nursery)]
extern crate alloc;

use alloc::string::String;
use beskar_core::video::{
    Info, Pixel, PixelComponents, PixelFormat,
    writer::{CHAR_HEIGHT, CHAR_WIDTH, FramebufferWriter, LETTER_SPACING, LINE_SPACING},
};
use core::fmt::{self, Write};

/// A rectangle expressed in character cells.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CharRect {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

impl CharRect {
    #[must_use]
    #[inline]
    pub const fn new(x: u16, y: u16, width: u16, height: u16) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    #[must_use]
    #[inline]
    pub const fn top(&self) -> u16 {
        self.y
    }

    #[must_use]
    #[inline]
    pub const fn left(&self) -> u16 {
        self.x
    }

    #[must_use]
    #[inline]
    pub const fn bottom(&self) -> u16 {
        self.y + self.height
    }

    #[must_use]
    #[inline]
    pub const fn right(&self) -> u16 {
        self.x + self.width
    }

    #[must_use]
    #[inline]
    pub const fn inset(&self, amount: u16) -> Self {
        Self {
            x: self.x + amount,
            y: self.y + amount,
            width: self.width.saturating_sub(amount * 2),
            height: self.height.saturating_sub(amount * 2),
        }
    }
}

/// A theme for the text display.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Theme {
    pub foreground: PixelComponents,
    pub background: PixelComponents,
}

impl Theme {
    #[must_use]
    #[inline]
    pub const fn new(foreground: PixelComponents, background: PixelComponents) -> Self {
        Self {
            foreground,
            background,
        }
    }

    #[must_use]
    #[inline]
    pub const fn inverse(&self) -> Self {
        Self {
            foreground: self.background,
            background: self.foreground,
        }
    }

    #[must_use]
    #[inline]
    pub const fn white_on_black() -> Self {
        Self {
            foreground: PixelComponents::WHITE,
            background: PixelComponents::BLACK,
        }
    }
}

/// Border styling for ASCII boxes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BoxStyle {
    pub horizontal: char,
    pub vertical: char,
    pub corners: [char; 4], // TL, TR, BR, BL
}

impl BoxStyle {
    #[must_use]
    #[inline]
    pub const fn new(horizontal: char, vertical: char, corners: [char; 4]) -> Self {
        Self {
            horizontal,
            vertical,
            corners,
        }
    }

    #[must_use]
    #[inline]
    pub const fn classic() -> Self {
        Self {
            horizontal: '-',
            vertical: '|',
            corners: ['+', '+', '+', '+'],
        }
    }

    #[must_use]
    #[inline]
    pub const fn heavy() -> Self {
        Self {
            horizontal: '=',
            vertical: '|',
            corners: ['#', '#', '#', '#'],
        }
    }

    #[must_use]
    #[inline]
    pub const fn angled() -> Self {
        Self {
            horizontal: '~',
            vertical: '!',
            corners: ['/', '\\', '\\', '/'],
        }
    }
}

/// Simple helper to draw ASCII UI elements on a framebuffer in character space.
pub struct AsciiCanvas<'a> {
    writer: FramebufferWriter,
    buffer: &'a mut [Pixel],
    cols: u16,
    rows: u16,
    cell_w: u16,
    cell_h: u16,
    pixel_format: PixelFormat,
    theme: Theme,
}

/// Buffered text formatter for ASCII UI output.
pub struct TextFormatter {
    buffer: alloc::vec::Vec<u8>,
}

impl TextFormatter {
    #[must_use]
    #[inline]
    pub const fn new() -> Self {
        Self {
            buffer: alloc::vec::Vec::new(),
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    #[must_use]
    #[inline]
    pub fn as_str(&self) -> &str {
        // SAFETY: The buffer is only written through `Write`, which guarantees valid UTF-8.
        unsafe { core::str::from_utf8_unchecked(&self.buffer) }
    }
}

impl Default for TextFormatter {
    fn default() -> Self {
        Self::new()
    }
}

impl Write for TextFormatter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.buffer.extend_from_slice(s.as_bytes());
        Ok(())
    }
}

impl<'a> AsciiCanvas<'a> {
    #[must_use]
    pub fn new(info: Info, buffer: &'a mut [Pixel], theme: Theme) -> Self {
        let cell_w = CHAR_WIDTH + LETTER_SPACING;
        let cell_h = CHAR_HEIGHT + LINE_SPACING;
        let cols = info.width() / cell_w.max(1);
        let rows = info.height() / cell_h.max(1);

        let mut writer = FramebufferWriter::new(info);
        writer.set_color(theme.foreground);

        Self {
            writer,
            buffer,
            cols,
            rows,
            cell_w,
            cell_h,
            pixel_format: info.pixel_format(),
            theme,
        }
    }

    #[inline]
    pub const fn set_color(&mut self, color: PixelComponents) {
        self.writer.set_color(color);
    }

    #[inline]
    pub const fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
        self.set_color(theme.foreground);
    }

    #[inline]
    pub fn clear(&mut self, color: PixelComponents) {
        let pixel = Pixel::from_format(self.pixel_format, color);
        self.writer.clear_screen(self.buffer, pixel);
    }

    #[inline]
    pub fn clear_with_theme(&mut self) {
        self.clear(self.theme.background);
    }

    #[must_use]
    #[inline]
    pub const fn cols(&self) -> u16 {
        self.cols
    }

    #[must_use]
    #[inline]
    pub const fn rows(&self) -> u16 {
        self.rows
    }

    #[must_use]
    #[inline]
    pub const fn cell_height(&self) -> u16 {
        self.cell_h
    }

    #[must_use]
    #[inline]
    pub const fn cell_width(&self) -> u16 {
        self.cell_w
    }

    #[inline]
    pub fn write_line(&mut self, col: u16, row: u16, text: &str) {
        self.write_line_with_width(col, row, text, None);
    }

    #[inline]
    fn write_line_with_width(&mut self, col: u16, row: u16, text: &str, max_width: Option<u16>) {
        if row >= self.rows || col >= self.cols {
            return;
        }
        let available = self.cols.saturating_sub(col);
        let width = max_width.map_or(available, |w| w.min(available));
        if width == 0 {
            return;
        }

        let trimmed: String = text.chars().take(width as usize).collect();
        let x = col.saturating_mul(self.cell_w);
        let y = row.saturating_mul(self.cell_h);
        self.writer.write_str_at(self.buffer, x, y, &trimmed);
    }

    #[inline]
    pub fn write_line_centered(&mut self, row: u16, text: &str) {
        if row >= self.rows {
            return;
        }
        let text_len: u16 =
            u16::try_from(text.chars().count().min(self.cols as usize)).unwrap_or(self.cols);
        let start_col = self.cols.saturating_sub(text_len) / 2;
        self.write_line(start_col, row, text);
    }

    #[inline]
    pub fn h_rule(&mut self, row: u16, ch: char) {
        if row >= self.rows {
            return;
        }
        let line: String = core::iter::repeat_n(ch, self.cols as usize - 1).collect();
        self.write_line(0, row, &line);
    }

    #[inline]
    pub fn fill_box(&mut self, rect: CharRect, fill: char) {
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        let max_row = rect.bottom().min(self.rows);
        let max_col = rect.right().min(self.cols);

        for row in rect.y..max_row {
            let width = max_col.saturating_sub(rect.x);
            let line: String = core::iter::repeat_n(fill, width as usize).collect();
            self.write_line(rect.x, row, &line);
        }
    }

    #[inline]
    pub fn stroke_box(&mut self, rect: CharRect, style: &BoxStyle) {
        if rect.width < 2 || rect.height < 2 {
            return;
        }

        let right = rect.right().saturating_sub(1);
        let bottom = rect.bottom().saturating_sub(1);

        // Horizontal edges
        for col in rect.x + 1..right {
            self.write_cell(col, rect.y, style.horizontal);
            self.write_cell(col, bottom, style.horizontal);
        }

        // Vertical edges
        for row in rect.y + 1..bottom {
            self.write_cell(rect.x, row, style.vertical);
            self.write_cell(right, row, style.vertical);
        }

        // Corners
        self.write_cell(rect.x, rect.y, style.corners[0]);
        self.write_cell(right, rect.y, style.corners[1]);
        self.write_cell(right, bottom, style.corners[2]);
        self.write_cell(rect.x, bottom, style.corners[3]);
    }

    #[inline]
    pub fn write_cell(&mut self, col: u16, row: u16, ch: char) {
        if col >= self.cols || row >= self.rows {
            return;
        }
        let x = col.saturating_mul(self.cell_w);
        let y = row.saturating_mul(self.cell_h);
        self.writer.write_char_at(self.buffer, x, y, ch);
    }

    #[inline]
    pub fn v_rule(&mut self, col: u16, rows: core::ops::Range<u16>, ch: char) {
        for row in rows {
            self.write_cell(col, row, ch);
        }
    }

    #[must_use]
    pub fn format_line(&self, text: &str) -> String {
        let max_len = self.cols.saturating_sub(2) as usize;
        text.chars().take(max_len).collect::<String>()
    }

    #[inline]
    pub fn write_formatted(&mut self, col: u16, row: u16, text: &str, max_width: u16) {
        self.write_line_with_width(col, row, text, Some(max_width));
    }

    #[must_use]
    #[inline]
    pub const fn theme(&self) -> Theme {
        self.theme
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use beskar_core::video::{Info, PixelComponents};

    #[test]
    fn test_char_rect_new() {
        let rect = CharRect::new(10, 20, 30, 40);
        assert_eq!(rect.x, 10);
        assert_eq!(rect.y, 20);
        assert_eq!(rect.width, 30);
        assert_eq!(rect.height, 40);
    }

    #[test]
    fn test_char_rect_bottom() {
        let rect = CharRect::new(5, 10, 20, 15);
        assert_eq!(rect.bottom(), 25); // 10 + 15
    }

    #[test]
    fn test_char_rect_right() {
        let rect = CharRect::new(5, 10, 20, 15);
        assert_eq!(rect.right(), 25); // 5 + 20
    }

    #[test]
    fn test_char_rect_inset() {
        let rect = CharRect::new(10, 20, 50, 40);
        let inset = rect.inset(5);
        assert_eq!(inset.x, 15);
        assert_eq!(inset.y, 25);
        assert_eq!(inset.width, 40); // 50 - 10
        assert_eq!(inset.height, 30); // 40 - 10
    }

    #[test]
    fn test_char_rect_inset_saturating() {
        let rect = CharRect::new(10, 20, 10, 8);
        let inset = rect.inset(10);
        assert_eq!(inset.x, 20);
        assert_eq!(inset.y, 30);
        assert_eq!(inset.width, 0); // Saturates to 0
        assert_eq!(inset.height, 0); // Saturates to 0
    }

    #[test]
    fn test_theme_new() {
        let theme = Theme::new(PixelComponents::WHITE, PixelComponents::BLACK);
        assert_eq!(theme.foreground, PixelComponents::WHITE);
        assert_eq!(theme.background, PixelComponents::BLACK);
    }

    #[test]
    fn test_theme_inverse() {
        let theme = Theme::white_on_black();
        let inverted = theme.inverse();
        assert_eq!(inverted.foreground, PixelComponents::BLACK);
        assert_eq!(inverted.background, PixelComponents::WHITE);
    }

    #[test]
    fn test_theme_white_on_black() {
        let theme = Theme::white_on_black();
        assert_eq!(theme.foreground, PixelComponents::WHITE);
        assert_eq!(theme.background, PixelComponents::BLACK);
    }

    #[test]
    fn test_box_style_classic() {
        let style = BoxStyle::classic();
        assert_eq!(style.horizontal, '-');
        assert_eq!(style.vertical, '|');
        assert_eq!(style.corners, ['+', '+', '+', '+']);
    }

    #[test]
    fn test_box_style_heavy() {
        let style = BoxStyle::heavy();
        assert_eq!(style.horizontal, '=');
        assert_eq!(style.vertical, '|');
        assert_eq!(style.corners, ['#', '#', '#', '#']);
    }

    #[test]
    fn test_box_style_angled() {
        let style = BoxStyle::angled();
        assert_eq!(style.horizontal, '~');
        assert_eq!(style.vertical, '!');
        assert_eq!(style.corners, ['/', '\\', '\\', '/']);
    }

    #[test]
    fn test_text_formatter_new() {
        let formatter = TextFormatter::new();
        assert_eq!(formatter.as_str(), "");
    }

    #[test]
    fn test_text_formatter_write() {
        let mut formatter = TextFormatter::new();
        write!(formatter, "Hello").unwrap();
        assert_eq!(formatter.as_str(), "Hello");
        write!(formatter, " World").unwrap();
        assert_eq!(formatter.as_str(), "Hello World");
    }

    #[test]
    fn test_text_formatter_clear() {
        let mut formatter = TextFormatter::new();
        write!(formatter, "Test").unwrap();
        assert_eq!(formatter.as_str(), "Test");
        formatter.clear();
        assert_eq!(formatter.as_str(), "");
    }

    #[test]
    fn test_text_formatter_default() {
        let formatter = TextFormatter::default();
        assert_eq!(formatter.as_str(), "");
    }

    #[test]
    fn test_text_formatter_with_format_args() {
        let mut formatter = TextFormatter::new();
        write!(formatter, "Value: {}", 42).unwrap();
        assert_eq!(formatter.as_str(), "Value: 42");
    }

    // Helper function to create a test canvas
    fn create_test_canvas<'a>(width: u16, height: u16, buffer: &'a mut [Pixel]) -> AsciiCanvas<'a> {
        let size = u32::from(width) * u32::from(height) * 4;
        let info = Info::new(size, width, height, PixelFormat::Rgb, width, 4);
        let theme = Theme::white_on_black();
        AsciiCanvas::new(info, buffer, theme)
    }

    #[test]
    fn test_ascii_canvas_new() {
        let mut buffer = [Pixel::from_format(PixelFormat::Rgb, PixelComponents::BLACK); 800 * 600];
        let canvas = create_test_canvas(800, 600, &mut buffer);

        assert!(canvas.cols() > 0);
        assert!(canvas.rows() > 0);
        assert_eq!(canvas.theme(), Theme::white_on_black());
    }

    #[test]
    fn test_ascii_canvas_dimensions() {
        let mut buffer = [Pixel::from_format(PixelFormat::Rgb, PixelComponents::BLACK); 800 * 600];
        let canvas = create_test_canvas(800, 600, &mut buffer);

        let cell_w = CHAR_WIDTH + LETTER_SPACING;
        let cell_h = CHAR_HEIGHT + LINE_SPACING;
        let expected_cols = 800 / cell_w.max(1);
        let expected_rows = 600 / cell_h.max(1);

        assert_eq!(canvas.cols(), expected_cols);
        assert_eq!(canvas.rows(), expected_rows);
        assert_eq!(canvas.cell_width(), cell_w);
        assert_eq!(canvas.cell_height(), cell_h);
    }

    #[test]
    fn test_ascii_canvas_set_theme() {
        let mut buffer = [Pixel::from_format(PixelFormat::Rgb, PixelComponents::BLACK); 800 * 600];
        let mut canvas = create_test_canvas(800, 600, &mut buffer);

        let new_theme = Theme::new(
            PixelComponents::new(255, 0, 0),
            PixelComponents::new(0, 255, 0),
        );

        canvas.set_theme(new_theme);
        assert_eq!(canvas.theme(), new_theme);
    }

    #[test]
    fn test_ascii_canvas_format_line() {
        let mut buffer = [Pixel::from_format(PixelFormat::Rgb, PixelComponents::BLACK); 800 * 600];
        let canvas = create_test_canvas(800, 600, &mut buffer);

        let long_text = "a".repeat(1000);
        let formatted = canvas.format_line(&long_text);

        // Should be trimmed to fit canvas width minus 2
        assert!(formatted.len() <= (canvas.cols().saturating_sub(2) as usize));
    }

    #[test]
    fn test_ascii_canvas_format_line_short_text() {
        let mut buffer = [Pixel::from_format(PixelFormat::Rgb, PixelComponents::BLACK); 800 * 600];
        let canvas = create_test_canvas(800, 600, &mut buffer);

        let short_text = "Hello";
        let formatted = canvas.format_line(short_text);

        assert_eq!(formatted, short_text);
    }

    #[test]
    fn test_char_rect_equality() {
        let rect1 = CharRect::new(10, 20, 30, 40);
        let rect2 = CharRect::new(10, 20, 30, 40);
        let rect3 = CharRect::new(10, 20, 30, 41);

        assert_eq!(rect1, rect2);
        assert_ne!(rect1, rect3);
    }

    #[test]
    fn test_theme_equality() {
        let theme1 = Theme::white_on_black();
        let theme2 = Theme::white_on_black();
        let theme3 = Theme::new(PixelComponents::BLACK, PixelComponents::WHITE);

        assert_eq!(theme1, theme2);
        assert_ne!(theme1, theme3);
    }

    #[test]
    fn test_box_style_equality() {
        let style1 = BoxStyle::classic();
        let style2 = BoxStyle::classic();
        let style3 = BoxStyle::heavy();

        assert_eq!(style1, style2);
        assert_ne!(style1, style3);
    }

    #[test]
    fn test_char_rect_zero_dimensions() {
        let rect = CharRect::new(10, 20, 0, 0);
        assert_eq!(rect.width, 0);
        assert_eq!(rect.height, 0);
        assert_eq!(rect.bottom(), 20);
        assert_eq!(rect.right(), 10);
    }

    #[test]
    fn test_char_rect_inset_edge_case() {
        let rect = CharRect::new(0, 0, 5, 5);
        let inset = rect.inset(3);
        assert_eq!(inset.width, 0); // 5 - 6 saturates to 0
        assert_eq!(inset.height, 0);
    }

    #[test]
    fn test_text_formatter_multiple_writes() {
        let mut formatter = TextFormatter::new();
        for i in 0..5 {
            write!(formatter, "{}", i).unwrap();
        }
        assert_eq!(formatter.as_str(), "01234");
    }

    #[test]
    fn test_text_formatter_utf8_content() {
        let mut formatter = TextFormatter::new();
        write!(formatter, "Hello 世界 🦀").unwrap();
        assert_eq!(formatter.as_str(), "Hello 世界 🦀");
    }

    #[test]
    fn test_theme_custom_colors() {
        let fg = PixelComponents::new(128, 64, 32);
        let bg = PixelComponents::new(200, 150, 100);
        let theme = Theme::new(fg, bg);

        assert_eq!(theme.foreground, fg);
        assert_eq!(theme.background, bg);

        let inverted = theme.inverse();
        assert_eq!(inverted.foreground, bg);
        assert_eq!(inverted.background, fg);
    }
}
