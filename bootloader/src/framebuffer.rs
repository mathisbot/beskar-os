use noto_sans_mono_bitmap::{
    FontWeight, RasterHeight, RasterizedChar, get_raster, get_raster_width,
};
pub use uefi::proto::console::gop::PixelBitmask;
use x86_64::{PhysAddr, VirtAddr};

/// Represents a frambuffer.
/// 
/// This is the struct that is sent to the kernel.
#[derive(Debug)]
pub struct FrameBuffer {
    buffer_start: VirtAddr,
    info: FrameBufferInfo,
}

#[derive(Debug)]
/// Represents a framebuffer with physical addresses.
pub struct PhysicalFrameBuffer {
    pub start_addr: PhysAddr,
    pub info: FrameBufferInfo,
}

impl PhysicalFrameBuffer {
    #[must_use]
    #[inline]
    /// Creates a new framebuffer instance.
    ///
    /// ## Safety
    ///
    /// The given start address and info must describe a valid framebuffer.
    pub const unsafe fn new(start_addr: PhysAddr, info: FrameBufferInfo) -> Self {
        Self { start_addr, info }
    }

    #[must_use]
    #[inline]
    pub const unsafe fn to_framebuffer(&self, virt_addr: VirtAddr) -> FrameBuffer {
        unsafe { FrameBuffer::new(virt_addr, self.info) }
    }

    #[must_use]
    #[inline]
    /// Returns the start address of the framebuffer.
    ///
    /// You should always prefer to use the `buffer` method to access the framebuffer.
    pub const fn buffer_start(&self) -> PhysAddr {
        self.start_addr
    }

    #[must_use]
    #[inline]
    /// Returns layout and pixel format information of the framebuffer.
    pub const fn info(&self) -> FrameBufferInfo {
        self.info
    }

    #[must_use]
    #[inline]
    /// Access the raw bytes of the framebuffer as a mutable slice.
    pub const fn buffer_mut(&mut self) -> &mut [u8] {
        unsafe {
            core::slice::from_raw_parts_mut(
                self.buffer_start().as_u64() as *mut u8,
                self.info().size,
            )
        }
    }
}

impl FrameBuffer {
    #[must_use]
    #[inline]
    /// Creates a new framebuffer instance.
    ///
    /// ## Safety
    ///
    /// The given start address and info must describe a valid framebuffer.
    pub const unsafe fn new(start_addr: VirtAddr, info: FrameBufferInfo) -> Self {
        Self {
            buffer_start: start_addr,
            info,
        }
    }

    #[must_use]
    #[inline]
    /// Returns layout and pixel format information of the framebuffer.
    pub const fn info(&self) -> FrameBufferInfo {
        self.info
    }

    #[must_use]
    #[inline]
    /// Access the raw bytes of the framebuffer as a mutable slice.
    pub const fn buffer_mut(&mut self) -> &mut [u8] {
        unsafe {
            core::slice::from_raw_parts_mut(self.buffer_start.as_mut_ptr::<u8>(), self.info().size)
        }
    }
}

/// Describes the layout and pixel format of a framebuffer.
#[derive(Debug, Clone, Copy)]
pub struct FrameBufferInfo {
    /// The total size in bytes.
    pub size: usize,
    /// The width in pixels.
    pub width: usize,
    /// The height in pixels.
    pub height: usize,
    /// The color format of each pixel.
    pub pixel_format: PixelFormat,
    /// The number of bytes per pixel.
    pub bytes_per_pixel: usize,
    /// Number of "virtual" pixels between the start of a line and the start of the next.
    ///
    /// The stride must be used to compute the start address of a next line as some framebuffers
    /// use additional padding at the end of a line.
    pub stride: usize,
}

/// Represents a pixel format, that is the layout of the color channels in a pixel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PixelFormat {
    /// One byte red, one byte green, one byte blue.
    Rgb,
    /// One byte blue, one byte green, one byte red.
    Bgr,
    /// Unknown pixel format represented as a bitmask.
    Bitmask(PixelBitmask),
}

const LINE_SPACING: usize = 2;
const LETTER_SPACING: usize = 0;
const BORDER_PADDING: usize = 1;

const CHAR_HEIGHT: RasterHeight = RasterHeight::Size20;
const CHAR_WIDTH: usize = get_raster_width(FontWeight::Regular, CHAR_HEIGHT);
const BACKUP_CHAR: char = '�';

#[must_use]
/// Returns the raster of the given char,
/// backing up to a default char if the given char is not available.
fn get_raster_backed(c: char) -> RasterizedChar {
    get_raster(c, FontWeight::Regular, CHAR_HEIGHT)
        .unwrap_or_else(|| get_raster(BACKUP_CHAR, FontWeight::Regular, CHAR_HEIGHT).unwrap())
}

/// Allows logging text to a pixel-based framebuffer.
pub struct FrameBufferWriter {
    raw_framebuffer: &'static mut [u8],
    info: FrameBufferInfo,
    x: usize,
    y: usize,
}

impl FrameBufferWriter {
    #[must_use]
    pub fn new(framebuffer_slice: &'static mut [u8], info: FrameBufferInfo) -> Self {
        Self {
            raw_framebuffer: framebuffer_slice,
            info,
            x: 0,
            y: 0,
        }
    }

    #[inline]
    fn newline(&mut self) {
        self.y += CHAR_HEIGHT.val() + LINE_SPACING;
        self.carriage_return();
    }

    #[inline]
    fn carriage_return(&mut self) {
        self.x = BORDER_PADDING;
    }

    #[inline]
    pub fn clear(&mut self) {
        self.x = BORDER_PADDING;
        self.y = BORDER_PADDING;
        self.raw_framebuffer.fill(0);
    }

    /// Writes a single char to the framebuffer.
    ///
    /// Handles control characters (newline and carriage return).
    fn write_char(&mut self, c: char) {
        match c {
            '\n' => self.newline(),
            '\r' => self.carriage_return(),
            c => {
                if self.x + CHAR_WIDTH + BORDER_PADDING >= self.info.width {
                    self.newline();
                }
                if self.y + CHAR_HEIGHT.val() + LINE_SPACING + BORDER_PADDING >= self.info.height {
                    self.clear();
                }

                let rasterized_char = get_raster_backed(c);

                for (v, row) in rasterized_char.raster().iter().enumerate() {
                    for (u, byte) in row.iter().enumerate() {
                        // Skip black pixels for speed purposes.
                        // This is a bit of a hack, but it works because in the bootloader,
                        // the screen is black and the text is white.
                        if *byte == 0 {
                            continue;
                        }
                        self.write_pixel(self.x + u, self.y + v, *byte);
                    }
                }
                self.x += rasterized_char.width() + LETTER_SPACING;
            }
        }
    }

    fn write_pixel(&mut self, x: usize, y: usize, intensity: u8) {
        let color = match self.info.pixel_format {
            PixelFormat::Rgb | PixelFormat::Bgr => [intensity, intensity, intensity, 0],
            PixelFormat::Bitmask(bitmask) => {
                // Intensity is thrown away
                let is_on = intensity > u8::MAX / 2;

                let mut color = 0_u32;
                color |= if is_on { bitmask.red } else { 0 };
                color |= if is_on { bitmask.green } else { 0 };
                color |= if is_on { bitmask.blue } else { 0 };

                color.to_ne_bytes()
            }
        };

        let bytes_per_pixel = self.info.bytes_per_pixel;
        let byte_offset = (y * self.info.stride + x) * bytes_per_pixel;

        self.raw_framebuffer[byte_offset..(byte_offset + bytes_per_pixel)]
            .copy_from_slice(&color[..bytes_per_pixel]);
    }
}

impl core::fmt::Write for FrameBufferWriter {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for c in s.chars() {
            self.write_char(c);
        }
        Ok(())
    }
}
