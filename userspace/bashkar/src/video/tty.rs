use super::screen;
use crate::video::assets::banner::BANNER_HEIGHT;
use alloc::string::String;
use beskar_core::video::{
    Info,
    writer::{CHAR_HEIGHT, FramebufferWriter, LINE_SPACING},
};
use beskar_lib::io::keyboard::{self, KeyCode, KeyState};
use hyperdrive::locks::mcs::MUMcsLock;

static TTY: MUMcsLock<Tty> = MUMcsLock::uninit();

/// Shell prompt to display
const PROMPT: &str = "BeskarOS> ";

pub fn init() {
    TTY.init(Tty::new());
    with_tty(Tty::display_prompt);
}

pub struct Tty {
    writer: FramebufferWriter,
    /// Current raw input buffer
    input_buffer: String,
    /// Current cursor position
    cursor_pos: usize,
}

impl Default for Tty {
    fn default() -> Self {
        Self::new()
    }
}

impl Tty {
    #[expect(clippy::cast_possible_truncation, reason = "No truncation here")]
    /// The number of rows to skip before writing to the screen.
    const START_ROW: u16 = BANNER_HEIGHT as u16 + 5;

    #[must_use]
    /// # Panics
    ///
    /// Panics if the screen is too small to fit the banner.
    pub fn new() -> Self {
        let screen_info = *screen::screen_info();

        let new_info = Info::new(
            screen_info.size(),
            screen_info.width(),
            screen_info.height().checked_sub(Self::START_ROW).unwrap(),
            screen_info.pixel_format(),
            screen_info.stride(),
            screen_info.bytes_per_pixel(),
        );

        Self {
            writer: FramebufferWriter::new(new_info),
            input_buffer: String::new(),
            cursor_pos: 0,
        }
    }

    fn offset_buffer<'a>(screen_buffer: &'a mut [u8], screen_info: &Info) -> &'a mut [u8] {
        let screen_stride = usize::from(screen_info.stride());
        let bpp = usize::from(screen_info.bytes_per_pixel());

        &mut screen_buffer[usize::from(Self::START_ROW) * screen_stride * bpp..]
    }

    #[expect(clippy::missing_panics_doc, reason = "This is never going to panic")]
    /// Display the shell prompt
    pub fn display_prompt(&mut self) {
        super::screen::with_screen(|screen| {
            let screen_info = screen::screen_info();
            let buffer = Self::offset_buffer(screen.buffer_mut(), screen_info);
            let (prefix, pixel_buff, suffix) = unsafe { buffer.align_to_mut() };
            assert!(prefix.is_empty() && suffix.is_empty());

            for c in PROMPT.chars() {
                self.writer.write_char(pixel_buff, c);
            }

            let curr_row = self.writer.y() + u16::try_from(BANNER_HEIGHT).unwrap();
            let end_row = curr_row + CHAR_HEIGHT + LINE_SPACING;
            screen.flush(Some(curr_row..end_row));
        });
    }

    #[expect(clippy::missing_panics_doc, reason = "This is never going to panic")]
    /// Redraw the current input line
    pub fn redraw_line(&mut self) {
        super::screen::with_screen(|screen| {
            let screen_info = screen::screen_info();
            let buffer = Self::offset_buffer(screen.buffer_mut(), screen_info);
            let (prefix, pixel_buff, suffix) = unsafe { buffer.align_to_mut() };
            assert!(prefix.is_empty() && suffix.is_empty());

            // First clear the line with a carriage return
            self.writer.write_char(pixel_buff, '\r');

            // Then write the prompt
            for c in PROMPT.chars() {
                self.writer.write_char(pixel_buff, c);
            }

            // Then write the input text
            for c in self.input_buffer.chars() {
                self.writer.write_char(pixel_buff, c);
            }

            let curr_row = self.writer.y() + u16::try_from(BANNER_HEIGHT).unwrap();
            let end_row = curr_row + CHAR_HEIGHT + LINE_SPACING;
            screen.flush(Some(curr_row..end_row));
        });
    }

    #[must_use]
    #[inline]
    /// Get the current input line text
    pub fn get_input_line(&self) -> &str {
        &self.input_buffer
    }

    #[inline]
    /// Reset the input line
    pub fn reset_input(&mut self) {
        self.input_buffer.clear();
        self.cursor_pos = 0;
    }

    #[expect(clippy::missing_panics_doc, reason = "This is never going to panic")]
    /// Handle a key event
    ///
    /// Returns true if the line is complete (e.g. Enter was pressed)
    pub fn handle_key_event(&mut self, event: &keyboard::KeyEvent) -> bool {
        let key = event.key();
        let pressed = event.pressed();

        if pressed != KeyState::Pressed {
            return false;
        }

        match key {
            KeyCode::Backspace => {
                if self.cursor_pos > 0 {
                    self.cursor_pos -= 1;
                    self.input_buffer.pop();
                    // TODO: Remove the last character from the screen
                    self.redraw_line();
                }
                false
            }
            KeyCode::Enter => {
                super::screen::with_screen(|screen| {
                    let screen_info = screen::screen_info();
                    let buffer = Self::offset_buffer(screen.buffer_mut(), screen_info);
                    let (prefix, pixel_buff, suffix) = unsafe { buffer.align_to_mut() };
                    assert!(prefix.is_empty() && suffix.is_empty());

                    self.writer.write_char(pixel_buff, '\n');

                    let curr_row = self.writer.y() + u16::try_from(BANNER_HEIGHT).unwrap();
                    let end_row = curr_row + CHAR_HEIGHT + LINE_SPACING;
                    screen.flush(Some(curr_row..end_row));
                });
                true
            }
            k => {
                let c = k.as_char();
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
            let screen_info = screen::screen_info();
            let buffer = Self::offset_buffer(screen.buffer_mut(), screen_info);
            let (prefix, pixel_buff, suffix) = unsafe { buffer.align_to_mut() };
            assert!(prefix.is_empty() && suffix.is_empty());

            let mut line_count = 0_u16;
            for c in s.chars() {
                self.writer.write_char(pixel_buff, c);

                if c == '\n' {
                    line_count += 1;
                }
            }

            let curr_row = self.writer.y() + u16::try_from(BANNER_HEIGHT).unwrap();
            let start_row = curr_row - line_count * (CHAR_HEIGHT + LINE_SPACING);
            let end_row = curr_row + CHAR_HEIGHT + LINE_SPACING;
            screen.flush(Some(start_row..end_row));
        });
    }
}

#[inline]
pub fn with_tty<R, F: FnOnce(&mut Tty) -> R>(f: F) -> R {
    TTY.with_locked(f)
}
