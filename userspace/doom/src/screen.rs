use beskar_core::video::Info;
use beskar_lib::io::screen::FrameBuffer;
use hyperdrive::{locks::mcs::MUMcsLock, once::Once};

const SCREENWIDTH: usize = 320;
const SCREENHEIGHT: usize = 200;
const CHANNELS: usize = 4; // RGBA

static SCREEN: MUMcsLock<FrameBuffer> = MUMcsLock::uninit();
static SCREEN_INFO: Once<Info> = Once::uninit();

#[link(name = "puredoom", kind = "static")]
unsafe extern "C" {
    unsafe fn doom_get_framebuffer(channel: i32) -> *const u8;
}

/// Initialize the screen framebuffer
///
/// # Panics
///
/// Panics if the framebuffer cannot be opened.
pub fn init() {
    SCREEN.init(FrameBuffer::open().unwrap());
    SCREEN_INFO.call_once(|| SCREEN.with_locked(|screen| *screen.info()));
}

#[must_use]
#[inline]
/// Returns the screen info.
///
/// # Panics
///
/// Panics if the screen info has not been initialized yet.
fn screen_info() -> &'static Info {
    SCREEN_INFO.get().unwrap()
}

#[inline]
fn with_screen<R, F: FnOnce(&mut FrameBuffer) -> R>(f: F) -> R {
    SCREEN.with_locked(f)
}

/// Draw the Doom framebuffer to the screen
pub fn draw() {
    let fb_start = unsafe { doom_get_framebuffer(CHANNELS as i32) };

    let info = screen_info();
    let stride_bytes = usize::from(info.stride()) * usize::from(info.bytes_per_pixel());
    let row_size = SCREENWIDTH * CHANNELS;

    with_screen(|screen| {
        let dst = screen.buffer_mut().as_mut_ptr();
        let src = fb_start;

        for y in 0..SCREENHEIGHT {
            unsafe {
                core::ptr::copy_nonoverlapping(
                    src.add(y * row_size),
                    dst.add(y * stride_bytes),
                    row_size,
                );
            }
        }

        let _ = screen.flush_rows(0..SCREENHEIGHT as u16);
    });
}
