use super::{File, Read};
use crate::error::{FileResult, IoResult};
pub use beskar_core::drivers::keyboard::{KeyCode, KeyEvent, KeyModifiers, KeyState};
use beskar_core::process::{SleepHandle, WaitResult};
use core::{
    mem::MaybeUninit,
    sync::atomic::{AtomicU64, Ordering},
};

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
#[expect(clippy::must_use_candidate)]
/// Wait until the next keyboard event occurs.
///
/// Note that this function is allowed to spuriously return even if no keyboard event has
/// occurred; in that case, simply call it again.
pub fn wait_next_event() -> WaitResult {
    let sh = cached_handle();
    crate::sys::sc_wait_on_event(sh, 0)
}

#[inline]
#[expect(clippy::must_use_candidate)]
/// Wait until the next keyboard event occurs, or timeout expires.
///
/// Returns the wake reason as reported by the kernel.
pub fn wait_next_event_timeout(timeout: beskar_core::time::Duration) -> WaitResult {
    let sh = cached_handle();
    crate::sys::sc_wait_on_event(sh, timeout.total_micros())
}

#[must_use]
fn cached_handle() -> SleepHandle {
    static WAIT_HANDLE_CACHE: AtomicU64 = AtomicU64::new(0);

    let raw = WAIT_HANDLE_CACHE.load(Ordering::Acquire);
    if raw != 0 {
        return SleepHandle::from_raw(raw);
    }
    core::hint::cold_path();

    // FIXME: Maybe synchronize in the kernel, or init drivers before allowing user processes to run

    // Not cached, query the kernel for the wait handle
    let handle = loop {
        let mut payload = MaybeUninit::<SleepHandle>::uninit();
        let code = crate::sys::sc_query_config(
            beskar_core::syscall::consts::QUERY_KEYBOARD_WAIT_HANDLE,
            payload.as_mut_ptr().cast(),
            size_of::<SleepHandle>() as u64,
        );

        if code.is_success() {
            // Safety: We just initialized the payload
            break unsafe { payload.assume_init() };
        }
        core::hint::spin_loop();
    };
    let raw = handle.raw();

    if cfg!(debug_assertions) {
        let previous = WAIT_HANDLE_CACHE.swap(raw, Ordering::Release);
        assert!(
            previous == 0 || previous == raw,
            "Multiple different keyboard wait handles detected: {previous:#x} and {raw:#x}"
        );
    } else {
        WAIT_HANDLE_CACHE.store(raw, Ordering::Release);
    }

    handle
}
