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

/// Returns the screen info.
///
/// # Panics
///
/// Panics if the screen info has not been initialized yet.
fn screen_info() -> &'static Info {
    SCREEN_INFO.get().unwrap()
}

fn with_screen<R, F: FnOnce(&mut FrameBuffer) -> R>(f: F) -> R {
    SCREEN.with_locked(f)
}

/// Draw the Doom framebuffer to the screen
pub fn draw() {
    let fb_start = unsafe { doom_get_framebuffer(CHANNELS.try_into().unwrap()) };
    let fb_raw = core::ptr::slice_from_raw_parts(fb_start, SCREENWIDTH * SCREENHEIGHT * CHANNELS);

    let Some(fb) = (unsafe { fb_raw.as_ref() }) else {
        beskar_lib::println!("Warning: Doom framebuffer is not initialized");
        return;
    };

    let stride = usize::from(screen_info().stride());
    let bpp = usize::from(screen_info().bytes_per_pixel());
    let stride_bytes = stride * bpp;
    with_screen(|screen| {
        let mut buffer_mut = screen.buffer_mut();
        for row in fb.chunks_exact(SCREENWIDTH * CHANNELS) {
            buffer_mut[..SCREENWIDTH * CHANNELS].copy_from_slice(row);
            buffer_mut = &mut buffer_mut[stride_bytes..];
        }
        let _ = screen.flush_rows(0..u16::try_from(SCREENHEIGHT).unwrap());
    });
}
