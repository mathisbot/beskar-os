use beskar_core::video::Info;
use beskar_lib::io::screen::FrameBuffer;
use hyperdrive::{locks::mcs::MUMcsLock, once::Once};

static SCREEN: MUMcsLock<FrameBuffer> = MUMcsLock::uninit();
static SCREEN_INFO: Once<Info> = Once::uninit();

/// Initialize the screen framebuffer
///
/// # Panics
///
/// Panics if the framebuffer cannot be opened.
pub fn init() {
    SCREEN.init(FrameBuffer::open().unwrap());
    SCREEN_INFO.call_once(|| SCREEN.with_locked(|screen| *screen.info()));
}

/// Returns the screen info.
///
/// # Panics
///
/// Panics if the screen info has not been initialized yet.
pub fn screen_info() -> &'static Info {
    SCREEN_INFO.get().unwrap()
}

pub fn with_screen<R, F: FnOnce(&mut FrameBuffer) -> R>(f: F) -> R {
    SCREEN.with_locked(f)
}
