//! Video related types and functions.

pub mod writer;

/// Bitmask used to indicate which bits of a pixel represent a given color.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(C)]
pub struct PixelBitmask {
    /// The bits indicating the red channel.
    pub red: u32,
    /// The bits indicating the green channel.
    pub green: u32,
    /// The bits indicating the blue channel.
    pub blue: u32,
}

/// Represents a pixel format, that is the layout of the color channels in a pixel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PixelFormat {
    /// One byte red, one byte green, one byte blue.
    ///
    /// Usually takes up 4 bytes, NULL, red, green, blue
    Rgb,
    /// One byte blue, one byte green, one byte red.
    ///
    /// Usually takes up 4 bytes, NULL, blue, green, red
    Bgr,
    /// Unknown pixel format represented as a bitmask.
    ///
    /// Usually takes up 4 bytes, where the layout is defined by the bitmask
    Bitmask(PixelBitmask),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct Pixel(u32);

impl Pixel {
    pub const BLACK: Self = Self(0);
    pub const WHITE: Self = Self(u32::MAX);

    #[must_use]
    #[inline]
    pub fn from_format(format: PixelFormat, red: u8, green: u8, blue: u8) -> Self {
        match format {
            PixelFormat::Rgb => Self::new_rgb(red, green, blue),
            PixelFormat::Bgr => Self::new_bgr(red, green, blue),
            PixelFormat::Bitmask(_mask) => unimplemented!(),
        }
    }

    #[must_use]
    #[inline]
    fn new_rgb(red: u8, green: u8, blue: u8) -> Self {
        Self(((u32::from(blue)) << 16) | ((u32::from(green)) << 8) | u32::from(red))
    }

    #[must_use]
    #[inline]
    fn new_bgr(red: u8, green: u8, blue: u8) -> Self {
        Self(((u32::from(red)) << 16) | ((u32::from(green)) << 8) | u32::from(blue))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Info {
    /// The total size in bytes.
    size: usize,
    /// The width in pixels.
    width: usize,
    /// The height in pixels.
    height: usize,
    /// The color format of each pixel.
    pixel_format: PixelFormat,
    /// The number of bytes per pixel.
    ///
    /// Should be 4.
    bytes_per_pixel: usize,
    /// Number of "virtual" pixels between the start of a line and the start of the next.
    ///
    /// The stride must be used to compute the start address of a next line as some framebuffers
    /// use additional padding at the end of a line.
    stride: usize,
}

impl Info {
    #[must_use]
    #[inline]
    pub const fn new(
        size: usize,
        width: usize,
        height: usize,
        pixel_format: PixelFormat,
        bytes_per_pixel: usize,
        stride: usize,
    ) -> Self {
        Self {
            size,
            width,
            height,
            pixel_format,
            bytes_per_pixel,
            stride,
        }
    }

    #[must_use]
    #[inline]
    /// The total size in bytes.
    pub const fn size(&self) -> usize {
        self.size
    }

    #[must_use]
    #[inline]
    /// The width in pixels.
    ///
    /// For computations of line offset, use `stride` instead
    pub const fn width(&self) -> usize {
        self.width
    }

    #[must_use]
    #[inline]
    /// The height in pixels.
    pub const fn height(&self) -> usize {
        self.height
    }

    #[must_use]
    #[inline]
    /// The color format of each pixel.
    pub const fn pixel_format(&self) -> PixelFormat {
        self.pixel_format
    }

    #[must_use]
    #[inline]
    /// The number of bytes per pixel.
    pub const fn bytes_per_pixel(&self) -> usize {
        self.bytes_per_pixel
    }

    #[must_use]
    #[inline]
    /// Number of "virtual" pixels between the start of a line and the start of the next.
    ///
    /// The stride must be used to compute the start address of a next line as some framebuffers
    /// use additional padding at the end of a line.
    pub const fn stride(&self) -> usize {
        self.stride
    }
}
