use super::{File, Read};
use crate::error::{FileResult, IoResult};
pub use beskar_core::drivers::keyboard::{KeyCode, KeyEvent, KeyModifiers, KeyState};
use core::mem::size_of;

#[repr(align(8))]
struct KeyboardEventBuffer([u8; size_of::<u64>()]);
beskar_core::static_assert!(align_of::<KeyboardEventBuffer>() >= align_of::<u64>());

/// A keyboard event reader that provides buffered input
pub struct KeyboardReader {
    file: File,
}

impl KeyboardReader {
    const KEYBOARD_FILE: &'static str = "/dev/keyboard";

    /// Creates a new keyboard reader
    ///
    /// # Errors
    ///
    /// Returns an error if the keyboard device cannot be opened
    pub fn new() -> FileResult<Self> {
        Ok(Self {
            file: File::open(Self::KEYBOARD_FILE)?,
        })
    }

    /// Gets the next keyboard event, if any
    ///
    /// # Errors
    ///
    /// Returns an error if reading from the keyboard device fails
    pub fn next_event(&mut self) -> IoResult<Option<KeyEvent>> {
        let mut buffer = KeyboardEventBuffer([0; size_of::<u64>()]);
        let bytes_read = self.file.read(&mut buffer.0)?;

        if bytes_read == buffer.0.len() {
            let value = u64::from_ne_bytes(buffer.0);
            Ok(KeyEvent::unpack_option(value))
        } else {
            Ok(None)
        }
    }
}

#[must_use]
#[inline]
/// Poll the kernel to get keyboard events
pub fn poll_keyboard() -> Option<KeyEvent> {
    let mut reader = KeyboardReader::new().ok()?;
    reader.next_event().ok()?
}

#[inline]
/// Wait until the next keyboard event occurs.
///
/// Note that this function is allowed to spuriously return even if no keyboard event has
/// occurred; in that case, simply call it again.
pub fn wait_next_event() {
    crate::sys::sc_wait_on_event(
        beskar_core::process::SleepHandle::SLEEP_HANDLE_KEYBOARD_INTERRUPT,
    );
}
