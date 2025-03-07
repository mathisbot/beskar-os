// TODO: Move into a `video` module (driver?)

use beskar_core::video::{FrameBuffer, Info, Pixel};
use hyperdrive::locks::mcs::MUMcsLock;

static SCREEN: MUMcsLock<Screen> = MUMcsLock::uninit();

pub fn init(frame_buffer: &'static mut FrameBuffer) {
    let info = frame_buffer.info();
    assert_eq!(
        info.bytes_per_pixel(),
        4,
        "Only 32-bit pixels are supported"
    );
    let screen = Screen::new(frame_buffer.buffer_mut(), info);

    SCREEN.init(screen);

    crate::info!(
        "Screen initialized with resolution {}x{}",
        info.width(),
        info.height()
    );
}

pub struct Screen {
    raw_buffer: &'static mut [Pixel],
    info: Info,
}

impl Screen {
    #[must_use]
    #[inline]
    pub fn new(raw_buffer: &'static mut [u8], info: Info) -> Self {
        assert!(
            raw_buffer.len() % info.bytes_per_pixel() == 0,
            "Buffer size must be a multiple of the pixel size"
        );

        // Convert the buffer to a slice of Pixels
        // Safety: Framebuffer is page aligned, the pointer is therefore aligned
        assert_eq!(size_of::<Pixel>(), info.bytes_per_pixel());
        let raw_buffer = unsafe {
            core::slice::from_raw_parts_mut(
                raw_buffer.as_mut_ptr().cast(),
                raw_buffer.len() / info.bytes_per_pixel(),
            )
        };

        Self { raw_buffer, info }
    }

    #[must_use]
    #[inline]
    pub const fn info(&self) -> Info {
        self.info
    }

    #[must_use]
    #[inline]
    /// Returns a reference to the raw buffer.
    pub const fn buffer_mut(&mut self) -> &mut [Pixel] {
        self.raw_buffer
    }

    /// Clears the screen with the given pixel.
    pub fn clear(&mut self, pixel: Pixel) {
        self.raw_buffer.fill(pixel);
    }
}

pub fn with_screen<R, F: FnOnce(&mut Screen) -> R>(f: F) -> R {
    SCREEN.with_locked(f)
}
