use super::screen;
use crate::video::assets::banner::BANNER_HEIGHT;
use beskar_core::video::{
    Info,
    writer::{CHAR_HEIGHT, FramebufferWriter, LINE_SPACING},
};
use beskar_lib::io::keyboard::{self, KeyCode};
use hyperdrive::locks::mcs::MUMcsLock;

static TTY: MUMcsLock<Tty> = MUMcsLock::uninit();

pub fn init() {
    TTY.init(Tty::new());
}

pub struct Tty {
    writer: FramebufferWriter,
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
        }
    }

    fn offset_buffer<'a>(screen_buffer: &'a mut [u8], screen_info: &Info) -> &'a mut [u8] {
        let screen_stride = usize::from(screen_info.stride());
        let bpp = usize::from(screen_info.bytes_per_pixel());

        &mut screen_buffer[usize::from(Self::START_ROW) * screen_stride * bpp..]
    }

    #[expect(clippy::missing_panics_doc, reason = "This is never going to panic")]
    pub fn handle_key_event(&mut self, event: &keyboard::KeyEvent) {
        let (key, pressed) = (event.key(), event.pressed());
        if pressed == keyboard::KeyState::Pressed {
            super::screen::with_screen(|screen| {
                let screen_info = screen::screen_info();
                let buffer = Self::offset_buffer(screen.buffer_mut(), screen_info);

                let (prefix, pixel_buff, suffix) = unsafe { buffer.align_to_mut() };
                assert!(prefix.is_empty() && suffix.is_empty());
                match key {
                    KeyCode::Backspace => {
                        self.writer.write_char(pixel_buff, '\r');
                    }
                    KeyCode::Enter => {
                        self.writer.write_char(pixel_buff, '\n');
                    }
                    k => {
                        let c = k.as_char();

                        self.writer.write_char(pixel_buff, c);

                        let curr_row = self.writer.y() + u16::try_from(BANNER_HEIGHT).unwrap();
                        let end_row = curr_row + CHAR_HEIGHT + LINE_SPACING;
                        // screen.flush(None);
                        screen.flush(Some(curr_row..end_row));
                    }
                }
            });
        }
    }
}

#[inline]
pub fn with_tty<R, F: FnOnce(&mut Tty) -> R>(f: F) -> R {
    TTY.with_locked(f)
}
