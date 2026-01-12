use super::{screen, ui};
use alloc::string::String;
use beskar_core::video::{Info, Pixel, writer::FramebufferWriter};
use beskar_lib::error::IoResult;
use beskar_lib::io::keyboard::{self, KeyCode, KeyState};
use beskar_lib::io::screen::FrameBuffer;
use core::mem;
use hyperdrive::locks::mcs::MUMcsLock;

static TTY: MUMcsLock<Tty> = MUMcsLock::uninit();

/// Shell prompt to display
const PROMPT: &str = "BESKAR-OS > ";

pub fn init() {
    let layout = ui::layout();
    TTY.init(Tty::new(*layout));
    with_tty(Tty::display_prompt);
}

pub struct Tty {
    writer: FramebufferWriter,
    /// Inner console origin in pixels (absolute)
    left_px: u16,
    top_px: u16,
    /// Grid geometry
    inner_cols: u16,
    inner_rows: u16,
    cell_w: u16,
    line_h: u16,
    /// Row where the current editable line starts
    line_start_row: u16,
    /// Number of characters currently rendered on the active input line (prompt + buffer)
    rendered_len: usize,
    /// Current cursor position on the grid
    cursor_col: u16,
    cursor_row: u16,
    /// Current raw input buffer
    input_buffer: String,
    /// Current cursor position within the input buffer
    cursor_pos: usize,
    // Keyboard modifiers
    modifiers: keyboard::KeyModifiers,
}

impl Tty {
    const PROMPT_LEN: usize = PROMPT.len();

    #[must_use]
    #[inline]
    const fn cell_to_pixel(&self, col: u16, row: u16) -> (u16, u16) {
        let x = self.left_px + col.saturating_mul(self.cell_w);
        let y = row.saturating_mul(self.line_h);
        (x, y)
    }

    #[must_use]
    /// # Panics
    ///
    /// Panics if the screen is too small to host the terminal area.
    pub fn new(layout: ui::UiLayout) -> Self {
        let screen_info = *screen::screen_info();

        // Compute inner console origin in pixels and grid dimensions, clamped to the screen
        let cell_w = layout.cell_width_px.max(1);
        let line_height = layout.cell_height_px.max(1);

        let max_cols = (screen_info.width() / cell_w).max(1);
        let max_rows = (screen_info.height() / line_height).max(1);

        let inner_left_col = layout.inner_left_col.min(max_cols - 1);
        let inner_cols = layout
            .inner_width_cols
            .min(max_cols.saturating_sub(inner_left_col))
            .max(1);

        let text_start_row = layout.text_start_row.min(max_rows - 1);
        let inner_rows = layout
            .inner_height_rows
            .min(max_rows.saturating_sub(text_start_row))
            .max(1);

        let left_px = inner_left_col.saturating_mul(cell_w);
        let top_px = text_start_row.saturating_mul(line_height);

        // Writer view starts at top_px (row-based view); width remains full stride
        let stride_bytes =
            usize::from(screen_info.stride()) * usize::from(screen_info.bytes_per_pixel());
        let remaining_rows = screen_info.height().saturating_sub(top_px).max(1);
        let view_size = stride_bytes * usize::from(remaining_rows);

        let new_info = Info::new(
            view_size.try_into().unwrap(),
            screen_info.width(),
            remaining_rows,
            screen_info.pixel_format(),
            screen_info.stride(),
            screen_info.bytes_per_pixel(),
        );

        let mut writer = FramebufferWriter::new(new_info);
        writer.set_color(ui::TEXT_COLOR);

        Self {
            writer,
            left_px,
            top_px,
            inner_cols,
            inner_rows,
            cell_w,
            line_h: line_height,
            line_start_row: 0,
            rendered_len: 0,
            cursor_col: 0,
            cursor_row: 0,
            input_buffer: String::new(),
            cursor_pos: 0,
            modifiers: keyboard::KeyModifiers::new(),
        }
    }

    /// Display the shell prompt
    ///
    /// # Panics
    ///
    /// Panics if flushing the framebuffer to the kernel framebuffer device fails.
    pub fn display_prompt(&mut self) {
        super::screen::with_screen(|screen| {
            let mut view = screen.view_from_row(self.top_px);
            let pixels = view.pixels_mut();

            self.cursor_col = 0;
            self.line_start_row = self.cursor_row;
            self.rendered_len = 0;

            self.write_span(pixels.as_mut(), PROMPT);
            self.rendered_len = Self::PROMPT_LEN;

            let rows = self.rows_spanned(self.rendered_len);
            self.flush_from_line(screen, self.line_start_row, rows)
                .unwrap();
        });
    }

    #[expect(clippy::missing_panics_doc, reason = "This is never going to panic")]
    /// Redraw the current input line
    pub fn redraw_line(&mut self) {
        super::screen::with_screen(|screen| {
            let mut view = screen.view_from_row(self.top_px);
            let pixels = view.pixels_mut();

            let prompt_len = Self::PROMPT_LEN;
            let input_len = self.input_buffer.chars().count();
            let new_len = prompt_len + input_len;
            let max_len = self.rendered_len.max(new_len);
            let rows_to_clear = self.rows_spanned(max_len);

            // Clear the previously rendered line region to avoid leftover glyphs
            self.clear_rows(pixels.as_mut(), self.line_start_row, rows_to_clear);

            // Redraw prompt + input from the tracked start row
            self.cursor_row = self.line_start_row;
            self.cursor_col = 0;

            // Write prompt
            self.write_span(pixels.as_mut(), PROMPT);

            // Write input buffer
            let input_copy = self.input_buffer.clone();
            self.write_span(pixels.as_mut(), &input_copy);

            self.rendered_len = new_len;
            self.flush_from_line(screen, self.line_start_row, rows_to_clear)
                .unwrap();
        });
    }

    #[expect(clippy::missing_panics_doc, reason = "This is never going to panic")]
    /// Clear the terminal screen
    pub fn clear_screen(&mut self) {
        super::screen::with_screen(|screen| {
            let mut view = screen.view_from_row(self.top_px);
            let pixels = view.pixels_mut();

            // Clear all rows in the inner console
            self.clear_rows(pixels.as_mut(), 0, self.inner_rows);

            // Reset cursor to top
            self.cursor_row = 0;
            self.cursor_col = 0;
            self.line_start_row = 0;
            self.rendered_len = 0;

            // Flush the entire terminal area
            self.flush_from_line(screen, 0, self.inner_rows).unwrap();
        });
    }

    #[must_use]
    #[inline]
    /// Get the current input line text
    pub fn get_input_line(&self) -> &str {
        &self.input_buffer
    }

    #[must_use]
    #[inline]
    /// Take ownership of the current input line, leaving the buffer empty.
    pub fn drain_input_line(&mut self) -> String {
        self.cursor_pos = 0;
        mem::take(&mut self.input_buffer)
    }

    #[inline]
    /// Reset the input line
    pub fn reset_input(&mut self) {
        self.input_buffer.clear();
        self.cursor_pos = 0;
    }

    /// Handle a key event
    ///
    /// Returns true if the line is complete (e.g. Enter was pressed)
    pub fn handle_key_event(&mut self, event: &keyboard::KeyEvent) -> bool {
        let key = event.key();
        let pressed = event.pressed();

        if pressed != KeyState::Pressed
            // Modifiers keys still need to be handled when released
            && !matches!(
                key,
                KeyCode::ShiftLeft
                    | KeyCode::ShiftRight
                    | KeyCode::CtrlLeft
                    | KeyCode::CtrlRight
                    | KeyCode::AltLeft
                    | KeyCode::AltRight
            )
        {
            return false;
        }

        match key {
            KeyCode::Backspace => {
                if self.cursor_pos > 0 {
                    self.cursor_pos -= 1;
                    if self.cursor_pos < self.input_buffer.len() {
                        self.input_buffer.remove(self.cursor_pos);
                    } else {
                        self.input_buffer.pop();
                    }
                    self.redraw_line();
                }
                false
            }
            KeyCode::Enter => {
                self.newline_and_flush();
                true
            }
            KeyCode::CapsLock => {
                self.modifiers
                    .set_caps_locked(!self.modifiers.is_caps_locked());
                false
            }
            KeyCode::ShiftLeft | KeyCode::ShiftRight => {
                self.modifiers.set_shifted(pressed == KeyState::Pressed);
                false
            }
            KeyCode::CtrlLeft | KeyCode::CtrlRight => {
                self.modifiers.set_ctrled(pressed == KeyState::Pressed);
                false
            }
            KeyCode::AltLeft | KeyCode::AltRight => {
                self.modifiers.set_alted(pressed == KeyState::Pressed);
                false
            }
            k => {
                let c = k.as_char(self.modifiers);
                if c != '\0' {
                    self.input_buffer.insert(self.cursor_pos, c);
                    self.cursor_pos += 1;
                    self.redraw_line();
                }
                false
            }
        }
    }

    #[expect(clippy::missing_panics_doc, reason = "This is never going to panic")]
    /// Write a string to the terminal
    pub fn write_str(&mut self, s: &str) {
        super::screen::with_screen(|screen| {
            let mut view = screen.view_from_row(self.top_px);
            let pixels = view.pixels_mut();
            let start_row = self.cursor_row;
            let mut rows_touched: u16 = 1;

            for c in s.chars() {
                match c {
                    '\n' => {
                        self.advance_line();
                        rows_touched = rows_touched.saturating_add(1);
                    }
                    ch => {
                        if self.ensure_fit_before_write() {
                            rows_touched = rows_touched.saturating_add(1);
                        }
                        let (x, y_rel) = self.cell_to_pixel(self.cursor_col, self.cursor_row);
                        self.writer.write_char_at(pixels.as_mut(), x, y_rel, ch);
                        if self.advance_cursor_cell() {
                            rows_touched = rows_touched.saturating_add(1);
                        }
                    }
                }
            }

            self.rendered_len = 0; // output invalidates any editable line rendering
            self.flush_from_line(screen, start_row, rows_touched)
                .unwrap();
        });
    }

    #[inline]
    const fn advance_cursor_cell(&mut self) -> bool {
        self.cursor_col += 1;
        if self.cursor_col >= self.inner_cols {
            self.advance_line();
            true
        } else {
            false
        }
    }

    #[inline]
    const fn advance_line(&mut self) {
        self.cursor_row = (self.cursor_row + 1) % self.inner_rows;
        self.cursor_col = 0;
    }

    const fn ensure_fit_before_write(&mut self) -> bool {
        if self.cursor_col >= self.inner_cols {
            self.advance_line();
            return true;
        }
        false
    }

    fn newline_and_flush(&mut self) {
        super::screen::with_screen(|screen| {
            let start_row = self.cursor_row;
            self.advance_line();
            self.flush_from_line(screen, start_row, 1).unwrap();
        });
    }

    fn write_span(&mut self, pixels: &mut [Pixel], text: &str) {
        for ch in text.chars() {
            let _ = self.ensure_fit_before_write();
            let (x, y_rel) = self.cell_to_pixel(self.cursor_col, self.cursor_row);
            self.writer.write_char_at(pixels, x, y_rel, ch);
            let _ = self.advance_cursor_cell();
        }
    }

    fn clear_rows(&mut self, pixels: &mut [Pixel], start_row: u16, rows: u16) {
        if rows == 0 {
            return;
        }

        let rows = rows.min(self.inner_rows);
        for offset in 0..rows {
            let row = (start_row + offset) % self.inner_rows;
            for col in 0..self.inner_cols {
                let (x, y_rel) = self.cell_to_pixel(col, row);
                self.writer.write_char_at(pixels, x, y_rel, ' ');
            }
        }
    }

    #[inline]
    fn rows_spanned(&self, len: usize) -> u16 {
        if self.inner_cols == 0 || len == 0 {
            return 1;
        }
        let cols = self.inner_cols as usize;
        let rows = len.div_ceil(cols);
        u16::try_from(rows.min(self.inner_rows as usize)).unwrap_or(self.inner_rows)
    }

    fn flush_from_line(&self, screen: &mut FrameBuffer, start_row: u16, rows: u16) -> IoResult<()> {
        if rows == 0 || self.inner_rows == 0 {
            return Ok(());
        }

        let start_row = start_row.min(self.inner_rows - 1);
        let rows = rows.min(self.inner_rows);

        let first_span = rows.min(self.inner_rows.saturating_sub(start_row));
        let start_abs = self
            .top_px
            .saturating_add(start_row.saturating_mul(self.line_h));
        let end_abs = start_abs.saturating_add(first_span.saturating_mul(self.line_h));
        screen.flush_rows(start_abs..end_abs)?;

        if rows > first_span {
            let wrap_rows = rows - first_span;
            let wrap_end = self
                .top_px
                .saturating_add(wrap_rows.saturating_mul(self.line_h));
            screen.flush_rows(self.top_px..wrap_end)?;
        }

        Ok(())
    }
}

#[inline]
pub fn with_tty<R, F: FnOnce(&mut Tty) -> R>(f: F) -> R {
    TTY.with_locked(f)
}
