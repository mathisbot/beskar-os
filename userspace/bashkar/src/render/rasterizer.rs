use beskar_core::video::{
    Pixel,
    writer::{CHAR_HEIGHT, CHAR_WIDTH, FramebufferWriter, LETTER_SPACING, LINE_SPACING},
};

use super::compositor::Screen;
use crate::buffer::{Cell, TermBuffer};
use crate::theme;

/// Cell dimensions in pixels.
pub const CELL_W: u16 = CHAR_WIDTH + LETTER_SPACING;
pub const CELL_H: u16 = CHAR_HEIGHT + LINE_SPACING;

/// Converts the logical cell buffer into pixels and presents them.
pub struct Rasterizer {
    screen: Screen,
    writer: FramebufferWriter,
    cols: u16,
    rows: u16,
    status_rows: u16,
}

impl Rasterizer {
    /// Create a fullscreen rasterizer, reserving `status_rows` at the top.
    #[must_use]
    pub fn new(status_rows: u16) -> Self {
        let screen = Screen::fullscreen();
        let info = *screen.info();
        let cols = info.width() / CELL_W.max(1);
        let rows = info.height() / CELL_H.max(1);
        let writer = FramebufferWriter::new(info);

        Self {
            screen,
            writer,
            cols,
            rows,
            status_rows,
        }
    }

    /// Total columns available.
    #[must_use]
    #[inline]
    pub const fn cols(&self) -> u16 {
        self.cols
    }

    /// Rows reserved for the status bar.
    #[must_use]
    #[inline]
    pub const fn status_rows(&self) -> u16 {
        self.status_rows
    }

    /// Rows available for the terminal content (below the status bar).
    #[must_use]
    #[inline]
    pub const fn content_rows(&self) -> u16 {
        self.rows.saturating_sub(self.status_rows)
    }

    /// Render the status bar at the top of the screen.
    pub fn draw_status_bar(&mut self, terminal_name: &str, line_count: usize) {
        let format = self.screen.info().pixel_format();
        let stride = self.screen.info().stride() as usize;
        let bar_pixel = Pixel::from_format(format, theme::BG_STATUS_BAR);
        let bar_height = self.status_rows as usize * CELL_H as usize;
        let bar_end = bar_height * stride;
        let cols = self.cols;
        let content_rows = self.content_rows();

        let pixels = self.screen.pixels_mut();
        for px in &mut pixels[..bar_end] {
            *px = bar_pixel;
        }

        self.writer.set_bg_color(theme::BG_STATUS_BAR);
        self.writer.set_color(theme::STATUS_LABEL);
        self.writer.write_str_at(pixels, CELL_W, 0, terminal_name);
        self.writer.set_color(theme::STATUS_FG);
        self.writer.write_str(pixels, "  |  command shell");

        let mut num_buf = [0u8; 40];
        let right_text = format_right_status(line_count, cols, content_rows, &mut num_buf);
        #[expect(clippy::cast_possible_truncation)]
        let text_cols = right_text.len() as u16;
        let start_col = cols.saturating_sub(text_cols + 1);
        self.writer.set_color(theme::STATUS_FG);
        let rpx = start_col * CELL_W;
        self.writer.write_str_at(pixels, rpx, 0, right_text);
        self.writer.set_bg_color(theme::BG_PRIMARY);
    }

    /// Render only the dirty rows from the buffer.
    pub fn draw_buffer(&mut self, buf: &TermBuffer, cursor_visible: bool) {
        let content_rows = self.content_rows();
        let format = self.screen.info().pixel_format();
        let stride = self.screen.info().stride() as usize;
        let cols = self.cols;
        let status_rows = self.status_rows;

        let pixels = self.screen.pixels_mut();

        for vrow in 0..content_rows.min(buf.rows()) {
            if !buf.is_row_dirty(vrow) {
                continue;
            }

            let screen_row = vrow + status_rows;
            let py_start = screen_row as usize * CELL_H as usize;

            let bg_pixel = Pixel::from_format(format, theme::BG_PRIMARY);
            for dy in 0..CELL_H as usize {
                let row_start = (py_start + dy) * stride;
                let row_end = row_start + (cols as usize * CELL_W as usize).min(stride);
                for px in &mut pixels[row_start..row_end] {
                    *px = bg_pixel;
                }
            }

            let line = buf.viewport_line(vrow);
            for (col_idx, cell) in line.iter().enumerate() {
                #[expect(clippy::cast_possible_truncation)]
                let col_u16 = col_idx as u16;
                if col_u16 >= cols {
                    break;
                }
                Self::render_cell_static(
                    &mut self.writer,
                    pixels,
                    format,
                    stride,
                    col_u16,
                    screen_row,
                    cell,
                );
            }
        }

        if cursor_visible && let Some(vrow) = buf.cursor_viewport_row() {
            let ccol = buf.cursor_col();
            if ccol < cols {
                Self::render_cursor_static(pixels, format, stride, ccol, vrow + status_rows);
            }
        }
    }

    /// Flush a specific row region.
    ///
    /// # Panics
    ///
    /// Panics if the compositor rejects the presentation.
    pub fn present_rows(&self, first_row: u16, count: u16) {
        let y = first_row * CELL_H;
        let h = count * CELL_H;
        self.screen
            .present_region(0, y, self.screen.info().width(), h)
            .expect("Failed to present region");
    }

    /// Fill a cell-sized rectangle at (col, row) with `color`.
    fn fill_cell_rect(
        pixels: &mut [Pixel],
        format: beskar_core::video::PixelFormat,
        stride: usize,
        col: u16,
        row: u16,
        color: beskar_core::video::PixelComponents,
    ) {
        let px = col * CELL_W;
        let py = row * CELL_H;
        let pixel = Pixel::from_format(format, color);
        for dy in 0..CELL_H as usize {
            for dx in 0..CELL_W as usize {
                let idx = (py as usize + dy) * stride + (px as usize + dx);
                if let Some(p) = pixels.get_mut(idx) {
                    *p = pixel;
                }
            }
        }
    }

    fn render_cell_static(
        writer: &mut FramebufferWriter,
        pixels: &mut [Pixel],
        format: beskar_core::video::PixelFormat,
        stride: usize,
        col: u16,
        row: u16,
        cell: &Cell,
    ) {
        if cell.ch == ' ' && cell.style.bg == theme::BG_PRIMARY {
            return;
        }

        if cell.style.bg != theme::BG_PRIMARY {
            Self::fill_cell_rect(pixels, format, stride, col, row, cell.style.bg);
        }

        if cell.ch != ' ' {
            let px = col * CELL_W;
            let py = row * CELL_H;
            writer.set_bg_color(cell.style.bg);
            writer.set_color(cell.style.fg);
            writer.write_char_at(pixels, px, py, cell.ch);
        }
    }

    fn render_cursor_static(
        pixels: &mut [Pixel],
        format: beskar_core::video::PixelFormat,
        stride: usize,
        col: u16,
        row: u16,
    ) {
        Self::fill_cell_rect(pixels, format, stride, col, row, theme::BG_CURSOR);
    }
}

/// Format the right-aligned status text into a scratch buffer.
fn format_right_status(line_count: usize, cols: u16, rows: u16, scratch: &mut [u8; 40]) -> &str {
    use core::fmt::Write;
    struct SliceWriter<'b> {
        buf: &'b mut [u8],
        pos: usize,
    }
    impl Write for SliceWriter<'_> {
        fn write_str(&mut self, s: &str) -> core::fmt::Result {
            let bytes = s.as_bytes();
            let space = self.buf.len() - self.pos;
            let n = bytes.len().min(space);
            self.buf[self.pos..self.pos + n].copy_from_slice(&bytes[..n]);
            self.pos += n;
            Ok(())
        }
    }
    let mut w = SliceWriter {
        buf: scratch,
        pos: 0,
    };
    let _ = write!(w, "L:{line_count}  {cols}x{rows}");
    let len = w.pos;
    core::str::from_utf8(&scratch[..len]).unwrap_or("?")
}
