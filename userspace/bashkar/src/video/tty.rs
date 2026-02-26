use super::{screen, ui};
use alloc::string::String;
use beskar_core::video::{Pixel, writer::FramebufferWriter};
use beskar_lib::io::keyboard::{self, KeyCode, KeyState};
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
    /// Grid geometry (in character cells)
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
    pub fn new(layout: ui::UiLayout) -> Self {
        let tty_info = *screen::tty_surface_info();
        let cell_w = layout.cell_width_px.max(1);
        let line_h = layout.cell_height_px.max(1);
        let inner_cols = (tty_info.width() / cell_w).max(1);
        let inner_rows = (tty_info.height() / line_h).max(1);
        let mut writer = FramebufferWriter::new(tty_info);
        writer.set_color(ui::TEXT_COLOR);
        Self {
            writer,
            inner_cols,
            inner_rows,
            cell_w,
            line_h,
            line_start_row: 0,
            rendered_len: 0,
            cursor_col: 0,
            cursor_row: 0,
            input_buffer: String::new(),
            cursor_pos: 0,
            modifiers: keyboard::KeyModifiers::new(),
        }
    }

    #[inline]
    const fn cell_to_pixel(&self, col: u16, row: u16) -> (u16, u16) {
        (
            col.saturating_mul(self.cell_w),
            row.saturating_mul(self.line_h),
        )
    }

    #[inline]
    const fn advance_cursor_cell(&mut self) {
        self.cursor_col += 1;
        if self.cursor_col >= self.inner_cols {
            self.advance_line();
        }
    }

    #[inline]
    const fn advance_line(&mut self) {
        self.cursor_row = (self.cursor_row + 1) % self.inner_rows;
        self.cursor_col = 0;
    }

    #[inline]
    const fn wrap_if_needed(&mut self) {
        if self.cursor_col >= self.inner_cols {
            self.advance_line();
        }
    }

    fn write_span(&mut self, pixels: &mut [Pixel], text: &str) {
        for ch in text.chars() {
            self.wrap_if_needed();
            let (x, y) = self.cell_to_pixel(self.cursor_col, self.cursor_row);
            self.writer.write_char_at(pixels, x, y, ch);
            self.advance_cursor_cell();
        }
    }

    /// Display the shell prompt at the current cursor row.
    ///
    /// # Panics
    ///
    /// Panics if flushing the surface fails.
    pub fn display_prompt(&mut self) {
        screen::with_tty_surface(|fb| {
            {
                let pixels = fb.pixels_mut();
                self.cursor_col = 0;
                self.line_start_row = self.cursor_row;
                self.rendered_len = 0;
                self.write_span(pixels, PROMPT);
                self.rendered_len = Self::PROMPT_LEN;
            }
            fb.flush_all().unwrap();
        });
    }

    /// Redraw the current input line (prompt + buffer), erasing any leftover
    /// characters if the line has shrunk.
    ///
    /// # Panics
    ///
    /// Panics if flushing the surface fails.
    fn redraw_input_line(&mut self) {
        screen::with_tty_surface(|fb| {
            let new_len = Self::PROMPT_LEN + self.input_buffer.chars().count();
            let old_len = self.rendered_len;
            {
                let pixels = fb.pixels_mut();
                self.cursor_row = self.line_start_row;
                self.cursor_col = 0;
                self.write_span(pixels, PROMPT);
                let input_copy = self.input_buffer.clone();
                self.write_span(pixels, &input_copy);
                // Cover any characters left over from a previously longer line.
                if new_len < old_len {
                    let padding = " ".repeat(old_len - new_len);
                    self.write_span(pixels, &padding);
                }
            }
            self.rendered_len = new_len;
            fb.flush_all().unwrap();
        });
    }

    /// Clear the terminal and redisplay the prompt.
    ///
    /// # Panics
    ///
    /// Panics if flushing the surface fails.
    pub fn clear_screen(&mut self) {
        screen::with_tty_surface(|fb| {
            fb.pixels_mut().fill(Pixel::BLACK);
            self.cursor_row = 0;
            self.cursor_col = 0;
            self.line_start_row = 0;
            self.rendered_len = 0;
            fb.flush_all().unwrap();
        });
        // Must be outside the closure: calling with_tty_surface again from inside
        // it would create aliased mutable borrows of the surface buffer.
        self.display_prompt();
    }

    /// Write program output to the terminal.
    ///
    /// # Panics
    ///
    /// Panics if flushing the surface fails.
    pub fn write_str(&mut self, s: &str) {
        screen::with_tty_surface(|fb| {
            {
                let pixels = fb.pixels_mut();
                for c in s.chars() {
                    match c {
                        '\n' => self.advance_line(),
                        ch => {
                            self.wrap_if_needed();
                            let (x, y) = self.cell_to_pixel(self.cursor_col, self.cursor_row);
                            self.writer.write_char_at(pixels, x, y, ch);
                            self.advance_cursor_cell();
                        }
                    }
                }
                self.rendered_len = 0;
            }
            fb.flush_all().unwrap();
        });
    }

    #[must_use]
    #[inline]
    pub fn get_input_line(&self) -> &str {
        &self.input_buffer
    }

    #[must_use]
    #[inline]
    /// Take the current input line, leaving the buffer empty.
    pub fn drain_input_line(&mut self) -> String {
        self.cursor_pos = 0;
        mem::take(&mut self.input_buffer)
    }

    #[inline]
    pub fn reset_input(&mut self) {
        self.input_buffer.clear();
        self.cursor_pos = 0;
    }

    /// Process a key event. Returns `true` when Enter is pressed.
    ///
    /// # Panics
    ///
    /// Panics if flushing the surface fails.
    pub fn handle_key_event(&mut self, event: &keyboard::KeyEvent) -> bool {
        let key = event.key();
        let pressed = event.pressed();

        if pressed != KeyState::Pressed
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
                    self.redraw_input_line();
                }
                false
            }
            KeyCode::Enter => {
                screen::with_tty_surface(|fb| {
                    self.advance_line();
                    fb.flush_all().unwrap();
                });
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
                    self.redraw_input_line();
                }
                false
            }
        }
    }
}

#[inline]
pub fn with_tty<R, F: FnOnce(&mut Tty) -> R>(f: F) -> R {
    TTY.with_locked(f)
}

impl core::fmt::Write for Tty {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.write_str(s);
        Ok(())
    }
}
