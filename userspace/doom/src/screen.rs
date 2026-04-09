use beskar_core::video::PixelFormat;
use beskar_lib::surface::Surface;
use hyperdrive::locks::mcs::MUMcsLock;
extern crate alloc;

const SCREENWIDTH: u16 = 320;
const SCREENHEIGHT: u16 = 200;
const CHANNELS: usize = 4; // RGBA/BGRA

/// Global Doom surface context
struct DoomContext {
    surface: Surface,
    format: PixelFormat,
    buffer: &'static mut [u8],
}

static DOOM_CONTEXT: MUMcsLock<Option<DoomContext>> = MUMcsLock::uninit();

#[link(name = "puredoom", kind = "static")]
unsafe extern "C" {
    unsafe fn doom_get_framebuffer(channel: i32) -> *const u8;
}

/// Initialize the screen framebuffer using the compositor Surface API
///
/// # Panics
///
/// Panics if surface creation fails.
pub fn init() {
    let size = u64::from(SCREENWIDTH) * u64::from(SCREENHEIGHT) * CHANNELS as u64;

    let buffer_ptr =
        beskar_lib::mem::mmap(size, None, beskar_lib::mem::MemoryProtection::ReadWrite)
            .expect("Failed to allocate framebuffer")
            .as_ptr();

    // SAFETY: We just allocated this buffer and we're the only owner
    let buffer: &'static mut [u8] = unsafe {
        core::slice::from_raw_parts_mut(
            buffer_ptr.cast(),
            (SCREENWIDTH as usize) * (SCREENHEIGHT as usize) * CHANNELS,
        )
    };

    // SAFETY: buffer is valid and has the correct size
    let surface = unsafe {
        Surface::create(SCREENWIDTH, SCREENHEIGHT, 0, 0, buffer_ptr.cast())
            .expect("Failed to create Doom compositor surface")
    };

    let format = beskar_lib::surface::query_screen_info()
        .expect("Failed to query screen info")
        .pixel_format();

    DOOM_CONTEXT.init(Some(DoomContext {
        surface,
        format,
        buffer,
    }));
}

/// Draw the Doom framebuffer to the screen
pub fn draw() {
    DOOM_CONTEXT.with_locked(|ctx| {
        if let Some(ctx) = ctx {
            // Get doom's framebuffer (RGBA format)
            let doom_fb = unsafe { doom_get_framebuffer(CHANNELS as i32) };

            if !doom_fb.is_null() {
                let pixel_count = (SCREENWIDTH as usize) * (SCREENHEIGHT as usize);
                let src = unsafe { core::slice::from_raw_parts(doom_fb, pixel_count * CHANNELS) };
                draw_raw(src, ctx.buffer, ctx.format);
            }

            // Present the entire surface to the compositor
            let _ = ctx.surface.present_all();
        }
    });
}

fn draw_raw(src: &[u8], dst: &mut [u8], format: PixelFormat) {
    debug_assert!(src.len() == dst.len());
    match format {
        PixelFormat::Argb8888 | PixelFormat::Xrgb8888 => {
            dst.copy_from_slice(src);
        }
        PixelFormat::Bgr8888 => {
            let pixel_count = src.len() / CHANNELS;
            // SAFETY: The mmap allocation and doom's framebuffer are both at least 4-byte aligned.
            let src_words =
                unsafe { core::slice::from_raw_parts(src.as_ptr().cast::<u32>(), pixel_count) };
            let dst_words = unsafe {
                core::slice::from_raw_parts_mut(dst.as_mut_ptr().cast::<u32>(), pixel_count)
            };
            for (d, &s) in dst_words.iter_mut().zip(src_words) {
                // Swap R and B channels
                *d = (s & 0xFF00_FF00) | (s.rotate_left(16) & 0x00FF_00FF);
            }
        }
        _ => {
            dst.chunks_exact_mut(CHANNELS).for_each(|p| {
                p.copy_from_slice(&[255u8, 0, 255, 255]);
            });
        }
    }
}
