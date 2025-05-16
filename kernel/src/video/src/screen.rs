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

pub struct Screen<'a> {
    raw_buffer: &'a mut [Pixel],
    info: Info,
}

impl<'a> Screen<'a> {
    #[must_use]
    #[inline]
    pub fn new(raw_buffer: &'a mut [u8], info: Info) -> Self {
        assert_eq!(size_of::<Pixel>(), info.bytes_per_pixel());

        // Safety: A `Pixel` is a `u32` and it is valid to pack 4 `u8`s into a `u32`.
        let (start_u8, pixel_buffer, end_u8) = unsafe { raw_buffer.align_to_mut::<Pixel>() };

        assert!(
            start_u8.is_empty() && end_u8.is_empty(),
            "Buffer is not aligned to Pixel"
        );

        Self {
            raw_buffer: pixel_buffer,
            info,
        }
    }
}

impl Screen<'_> {
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

#[inline]
pub fn with_screen<R, F: FnOnce(&mut Screen) -> R>(f: F) -> R {
    SCREEN.with_locked(f)
}
