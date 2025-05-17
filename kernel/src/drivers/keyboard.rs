use alloc::boxed::Box;
use beskar_core::drivers::keyboard::KeyEvent;
use core::mem::offset_of;
use hyperdrive::{
    once::Once,
    queues::mpsc::{Link, MpscQueue, Queueable},
};

static KEYBOARD_MANAGER: Once<KeyboardManager> = Once::uninit();

pub fn init() {
    KEYBOARD_MANAGER.call_once(KeyboardManager::new);
}

struct QueuedKeyEvent {
    event: KeyEvent,
    _link: Link<Self>,
}

impl Queueable for QueuedKeyEvent {
    type Handle = Box<Self>;

    unsafe fn capture(ptr: core::ptr::NonNull<Self>) -> Self::Handle {
        unsafe { Box::from_raw(ptr.as_ptr()) }
    }

    unsafe fn get_link(ptr: core::ptr::NonNull<Self>) -> core::ptr::NonNull<Link<Self>> {
        unsafe { ptr.byte_add(offset_of!(Self, _link)) }.cast()
    }

    fn release(r: Self::Handle) -> core::ptr::NonNull<Self> {
        let boxed_event = Box::into_raw(r);
        unsafe { core::ptr::NonNull::new_unchecked(boxed_event) }
    }
}

impl QueuedKeyEvent {
    #[must_use]
    #[inline]
    pub fn new(event: KeyEvent) -> Self {
        Self {
            event,
            _link: Link::default(),
        }
    }

    #[must_use]
    #[inline]
    pub const fn event(&self) -> KeyEvent {
        self.event
    }
}

pub struct KeyboardManager {
    event_queue: MpscQueue<QueuedKeyEvent>,
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
            event_queue: MpscQueue::new(Box::new(QueuedKeyEvent::new(KeyEvent::stub()))),
        }
    }
    #[inline]
    pub fn push_event(&self, event: KeyEvent) {
        let queued_event = Box::new(QueuedKeyEvent::new(event));
        self.event_queue.enqueue(queued_event);
    }

    #[must_use]
    #[inline]
    pub fn poll_event(&self) -> Option<KeyEvent> {
        self.event_queue.dequeue().map(|event| event.event())
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
