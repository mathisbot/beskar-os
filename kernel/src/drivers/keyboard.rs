use beskar_core::drivers::keyboard::KeyEvent;
use hyperdrive::{once::Once, queues::mpmc::MpmcQueue};

const QUEUE_SIZE: usize = 25;

static KEYBOARD_MANAGER: Once<KeyboardManager> = Once::uninit();

pub fn init() {
    KEYBOARD_MANAGER.call_once(KeyboardManager::new);
}

pub struct KeyboardManager {
    event_queue: MpmcQueue<QUEUE_SIZE, KeyEvent>,
}

impl Default for KeyboardManager {
    fn default() -> Self {
        Self::new()
    }
}

impl KeyboardManager {
    #[must_use]
    #[inline]
    pub fn new() -> Self {
        Self {
            event_queue: MpmcQueue::new(),
        }
    }

    #[inline]
    pub fn push_event(&self, event: KeyEvent) {
        let push_res = self.event_queue.try_push(event);
        #[cfg(debug_assertions)]
        if push_res.is_err() {
            // FIXME: Override old events instead of dropping new ones.
            video::debug!("Keyboard event queue is full, dropping event: {:?}", event);
        }
    }

    #[must_use]
    #[inline]
    pub fn poll_event(&self) -> Option<KeyEvent> {
        self.event_queue.pop()
    }
}

/// Operate on the keyboard manager.
///
/// Note that this function does not involve any locking.
pub fn with_keyboard_manager<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&KeyboardManager) -> R,
{
    KEYBOARD_MANAGER.get().map(f)
}

pub struct KeyboardDevice;

impl ::storage::KernelDevice for KeyboardDevice {
    fn read(&mut self, dst: &mut [u8], _offset: usize) -> Result<(), ::storage::BlockDeviceError> {
        let (prefix, dst, suffix) = unsafe { dst.align_to_mut::<u64>() };

        if !prefix.is_empty() || !suffix.is_empty() {
            return Err(::storage::BlockDeviceError::UnalignedAccess);
        }

        for block in dst.iter_mut() {
            let key_event = with_keyboard_manager(KeyboardManager::poll_event).flatten();
            *block = KeyEvent::pack_option(key_event);
        }

        Ok(())
    }

    fn write(&mut self, src: &[u8], _offset: usize) -> Result<(), ::storage::BlockDeviceError> {
        if src.is_empty() {
            Ok(())
        } else {
            Err(::storage::BlockDeviceError::Unsupported)
        }
    }
}
