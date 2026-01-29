//! Video related types and functions.

use crate::arch::VirtAddr;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct PixelComponents {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

impl PixelComponents {
    pub const WHITE: Self = Self {
        red: 0xFF,
        green: 0xFF,
        blue: 0xFF,
    };
    pub const BLACK: Self = Self {
        red: 0x00,
        green: 0x00,
        blue: 0x00,
    };

    pub const RED: Self = Self {
        red: 0xFF,
        green: 0x00,
        blue: 0x00,
    };
    pub const GREEN: Self = Self {
        red: 0x00,
        green: 0xFF,
        blue: 0x00,
    };
    pub const BLUE: Self = Self {
        red: 0x00,
        green: 0x00,
        blue: 0xFF,
    };

    pub const CYAN: Self = Self {
        red: 0x00,
        green: 0xFF,
        blue: 0xFF,
    };
    pub const MAGENTA: Self = Self {
        red: 0xFF,
        green: 0x00,
        blue: 0xFF,
    };
    pub const YELLOW: Self = Self {
        red: 0xFF,
        green: 0xFF,
        blue: 0x00,
    };

    pub const ORANGE: Self = Self {
        red: 0xFF,
        green: 0xA5,
        blue: 0x00,
    };

    #[must_use]
    #[inline]
    pub const fn new(red: u8, green: u8, blue: u8) -> Self {
        Self { red, green, blue }
    }
}

impl core::ops::Add<Self> for PixelComponents {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            red: self.red.saturating_add(rhs.red),
            green: self.green.saturating_add(rhs.green),
            blue: self.blue.saturating_add(rhs.blue),
        }
    }
}

impl core::ops::Mul<Self> for PixelComponents {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        Self {
            red: u8::try_from((u16::from(self.red) * u16::from(rhs.red) + 128) >> 8).unwrap(),
            green: u8::try_from((u16::from(self.green) * u16::from(rhs.green) + 128) >> 8).unwrap(),
            blue: u8::try_from((u16::from(self.blue) * u16::from(rhs.blue) + 128) >> 8).unwrap(),
        }
    }
}

impl Pixel {
    pub const BLACK: Self = Self(0);
    pub const WHITE: Self = Self(u32::MAX);

    #[must_use]
    #[inline]
    pub const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    #[must_use]
    #[inline]
    pub const fn to_raw(self) -> u32 {
        self.0
    }

    #[must_use]
    #[inline]
    pub fn from_format(format: PixelFormat, components: PixelComponents) -> Self {
        match format {
            PixelFormat::Rgb => Self::new_rgb(components),
            PixelFormat::Bgr => Self::new_bgr(components),
            PixelFormat::Bitmask(_mask) => todo!("Bitmask pixel format"),
        }
    }

    #[must_use]
    #[inline]
    pub fn new_rgb(components: PixelComponents) -> Self {
        Self(
            ((u32::from(components.blue)) << 16)
                | ((u32::from(components.green)) << 8)
                | u32::from(components.red),
        )
    }

    #[must_use]
    #[inline]
    pub fn new_bgr(components: PixelComponents) -> Self {
        Self(
            ((u32::from(components.red)) << 16)
                | ((u32::from(components.green)) << 8)
                | u32::from(components.blue),
        )
    }

    #[must_use]
    #[inline]
    pub fn components_by_format(self, format: PixelFormat) -> PixelComponents {
        match format {
            PixelFormat::Rgb => self.components_rgb(),
            PixelFormat::Bgr => self.components_bgr(),
            PixelFormat::Bitmask(_mask) => todo!("Bitmask pixel format"),
        }
    }

    #[must_use]
    #[inline]
    pub fn components_bgr(self) -> PixelComponents {
        let red = u8::try_from((self.0 >> 16) & 0xFF).unwrap();
        let green = u8::try_from((self.0 >> 8) & 0xFF).unwrap();
        let blue = u8::try_from(self.0 & 0xFF).unwrap();
        PixelComponents { red, green, blue }
    }

    #[must_use]
    #[inline]
    pub fn components_rgb(self) -> PixelComponents {
        let blue = u8::try_from((self.0 >> 16) & 0xFF).unwrap();
        let green = u8::try_from((self.0 >> 8) & 0xFF).unwrap();
        let red = u8::try_from(self.0 & 0xFF).unwrap();
        PixelComponents { red, green, blue }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct Info {
    /// The total size in bytes.
    size: u32,
    /// The width in pixels.
    width: u16,
    /// The height in pixels.
    height: u16,
    /// The color format of each pixel.
    pixel_format: PixelFormat,
    /// Number of "virtual" pixels between the start of a line and the start of the next.
    ///
    /// The stride must be used to compute the start address of a next line as some framebuffers
    /// use additional padding at the end of a line.
    stride: u16,
    /// The number of bytes per pixel.
    ///
    /// Should be 4.
    bytes_per_pixel: u8,
}

impl Info {
    #[must_use]
    #[inline]
    pub const fn new(
        size: u32,
        width: u16,
        height: u16,
        pixel_format: PixelFormat,
        stride: u16,
        bytes_per_pixel: u8,
    ) -> Self {
        Self {
            size,
            width,
            height,
            pixel_format,
            stride,
            bytes_per_pixel,
        }
    }

    #[must_use]
    #[inline]
    /// The total size in bytes.
    pub const fn size(&self) -> u32 {
        self.size
    }

    #[must_use]
    #[inline]
    /// The width in pixels.
    ///
    /// For computations of line offset, use `stride` instead
    pub const fn width(&self) -> u16 {
        self.width
    }

    #[must_use]
    #[inline]
    /// The height in pixels.
    pub const fn height(&self) -> u16 {
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
    pub const fn bytes_per_pixel(&self) -> u8 {
        self.bytes_per_pixel
    }

    #[must_use]
    #[inline]
    /// Number of "virtual" pixels between the start of a line and the start of the next.
    ///
    /// The stride must be used to compute the start address of a next line as some framebuffers
    /// use additional padding at the end of a line.
    pub const fn stride(&self) -> u16 {
        self.stride
    }
}

/// Represents a frambuffer.
///
/// This is the struct that is sent to the kernel.
#[derive(Debug)]
pub struct FrameBuffer {
    buffer_start: VirtAddr,
    info: Info,
}

impl FrameBuffer {
    #[must_use]
    #[inline]
    /// Creates a new framebuffer instance.
    ///
    /// # Safety
    ///
    /// The given start address and info must describe a valid framebuffer.
    pub const unsafe fn new(start_addr: VirtAddr, info: Info) -> Self {
        Self {
            buffer_start: start_addr,
            info,
        }
    }

    #[must_use]
    #[inline]
    /// Returns layout and pixel format information of the framebuffer.
    pub const fn info(&self) -> Info {
        self.info
    }

    #[must_use]
    #[inline]
    /// Access the raw bytes of the framebuffer as a mutable slice.
    pub fn buffer_mut(&mut self) -> &mut [u8] {
        unsafe {
            core::slice::from_raw_parts_mut(
                self.buffer_start.as_mut_ptr::<u8>(),
                usize::try_from(self.info().size()).unwrap(),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pixel_components_ops() {
        let pixel_components_1 = PixelComponents {
            red: 0x10,
            green: 0x20,
            blue: 0x30,
        };
        let pixel_components_2 = PixelComponents {
            red: 0x40,
            green: 0x50,
            blue: 0x60,
        };

        let sum = pixel_components_1 + pixel_components_2;
        assert_eq!(
            sum,
            PixelComponents {
                red: 0x50,
                green: 0x70,
                blue: 0x90,
            }
        );

        let mul = pixel_components_1 * pixel_components_2;
        assert_eq!(
            mul,
            PixelComponents {
                red: 0x04,
                green: 0x0A,
                blue: 0x12,
            }
        );
    }
}
