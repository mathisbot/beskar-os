use hyperdrive::locks::mcs::MUMcsLock;

pub mod pixel;
use pixel::PixelFormat;

static SCREEN: MUMcsLock<Screen> = MUMcsLock::uninit();

pub fn init(raw_buffer: &'static mut [u8], info: Info) {
    let screen = Screen::new(raw_buffer, info);
    SCREEN.init(screen);
}

pub struct Screen {
    raw_buffer: &'static mut [u8],
    info: Info,
}

#[derive(Debug, Clone, Copy)]
pub struct Info {
    /// The width in pixels.
    pub width: usize,
    /// The height in pixels.
    pub height: usize,
    /// The number of bytes per pixel.
    pub bytes_per_pixel: usize,
    /// Number of "virtual" pixels between the start of a line and the start of the next.
    ///
    /// The stride must be used to compute the start address of a next line as some framebuffers
    /// use additional padding at the end of a line.
    pub stride: usize,
    /// Format of the pixel data.
    pub pixel_format: PixelFormat,
}

impl From<bootloader::FrameBufferInfo> for Info {
    fn from(value: bootloader::FrameBufferInfo) -> Self {
        Self {
            width: value.width,
            height: value.height,
            bytes_per_pixel: value.bytes_per_pixel,
            stride: value.stride,
            pixel_format: match value.pixel_format {
                bootloader::PixelFormat::Rgb => PixelFormat::Rgb,
                bootloader::PixelFormat::Bgr => PixelFormat::Bgr,
                _ => unimplemented!("Unsupported pixel format"),
            },
        }
    }
}

impl Screen {
    #[must_use]
    #[inline]
    pub const fn new(raw_buffer: &'static mut [u8], info: Info) -> Self {
        Self { raw_buffer, info }
    }

    #[must_use]
    #[inline]
    pub const fn info(&self) -> Info {
        self.info
    }

    #[must_use]
    #[inline]
    pub fn buffer_mut(&mut self) -> &mut [u8] {
        self.raw_buffer
    }

    // TODO: Clear with pixel color, not byte
    /// Clears the screen with the given byte.
    pub fn clear(&mut self, byte: u8) {
        self.raw_buffer.fill(byte);
    }
}

pub fn with_screen<F, R>(f: F) -> R
where
    F: FnOnce(&mut Screen) -> R,
{
    SCREEN.with_locked(f)
}
