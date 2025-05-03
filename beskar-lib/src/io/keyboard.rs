use super::{close, open, read};
pub use beskar_core::drivers::keyboard::{KeyCode, KeyEvent, KeyState};

#[must_use]
#[inline]
/// Poll the kernel to get keyboard events
///
/// # Panics
///
/// Panics if any operation fails (open, read, close).
pub fn poll_keyboard() -> Option<KeyEvent> {
    const KEYBOARD_FILE: &str = "/dev/keyboard";

    // FIXME: This is very inefficient and faillible if some other process
    // is using the keyboard file.
    let handle = open(KEYBOARD_FILE).unwrap();

    let mut buffer = [0_u8; 8];
    let bytes_read = read(handle, &mut buffer, 0).unwrap();

    close(handle).unwrap();

    if bytes_read == buffer.len() {
        let value = u64::from_ne_bytes(buffer);
        KeyEvent::unpack_option(value)
    } else {
        None
    }
}
