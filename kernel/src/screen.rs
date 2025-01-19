// TODO: Move into a `video` module (driver?)

use bootloader::video::{FrameBuffer, FrameBufferInfo};
use hyperdrive::locks::mcs::MUMcsLock;

pub mod pixel;
use pixel::{PIXEL_SIZE, Pixel, PixelFormat};

static SCREEN: MUMcsLock<Screen> = MUMcsLock::uninit();

pub fn init(frame_buffer: &'static mut FrameBuffer) {
    let info = frame_buffer.info();
    assert_eq!(
        info.bytes_per_pixel(),
        PIXEL_SIZE,
        "Only 32-bit pixels are supported"
    );
    let screen = Screen::new(frame_buffer.buffer_mut(), info.into());

    SCREEN.init(screen);

    crate::info!(
        "Screen initialized with resolution {}x{}",
        info.width(),
        info.height()
    );
}

pub struct Screen {
    raw_buffer: &'static mut [u32],
    info: Info,
}

#[derive(Debug, Clone, Copy)]
pub struct Info {
    /// The width in pixels.
    pub width: usize,
    /// The height in pixels.
    pub height: usize,
    /// Number of "virtual" pixels between the start of a line and the start of the next.
    ///
    /// The stride must be used to compute the start address of a next line as some framebuffers
    /// use additional padding at the end of a line.
    pub stride: usize,
    /// Format of the pixel data.
    pub pixel_format: PixelFormat,
}

impl From<FrameBufferInfo> for Info {
    fn from(info: FrameBufferInfo) -> Self {
        Self {
            width: info.width(),
            height: info.height(),
            stride: info.stride(),
            pixel_format: match info.pixel_format() {
                bootloader::video::PixelFormat::Rgb => PixelFormat::Rgb,
                bootloader::video::PixelFormat::Bgr => PixelFormat::Bgr,
                _ => unimplemented!("Unsupported pixel format"),
            },
        }
    }
}

impl Screen {
    #[must_use]
    #[inline]
    pub fn new(raw_buffer: &'static mut [u8], info: Info) -> Self {
        assert!(
            raw_buffer.len() % PIXEL_SIZE == 0,
            "Buffer size must be a multiple of 4"
        );

        // Convert the buffer to a slice of u32
        // Safety: Framebuffer is page aligned, the pointer is therefore dword aligned
        let raw_buffer = unsafe {
            core::slice::from_raw_parts_mut(
                raw_buffer.as_mut_ptr().cast(),
                raw_buffer.len() / PIXEL_SIZE,
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
    pub fn buffer_mut(&mut self) -> &mut [u32] {
        self.raw_buffer
    }

    /// Clears the screen with the given pixel.
    pub fn clear(&mut self, pixel: Pixel) {
        self.raw_buffer.fill(pixel.into());
    }
}

pub fn with_screen<R, F: FnOnce(&mut Screen) -> R>(f: F) -> R {
    SCREEN.with_locked(f)
}
