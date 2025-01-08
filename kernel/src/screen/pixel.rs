pub const PIXEL_SIZE: usize = core::mem::size_of::<u32>();

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PixelFormat {
    Rgb,
    Bgr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Pixel {
    format: PixelFormat,
    red: u8,
    green: u8,
    blue: u8,
}

impl From<Pixel> for u32 {
    fn from(pixel: Pixel) -> Self {
        match pixel.format {
            PixelFormat::Rgb => {
                (Self::from(pixel.blue) << 16)
                    | (Self::from(pixel.green) << 8)
                    | Self::from(pixel.red)
            }
            PixelFormat::Bgr => {
                (Self::from(pixel.red) << 16)
                    | (Self::from(pixel.green) << 8)
                    | Self::from(pixel.blue)
            }
        }
    }
}

impl Pixel {
    #[must_use]
    #[inline]
    pub const fn from_u32(value: u32, format: PixelFormat) -> Self {
        #[allow(clippy::cast_possible_truncation)] // This is what we want!
        match format {
            PixelFormat::Rgb => Self {
                format,
                red: (value >> 24) as u8,
                green: (value >> 16) as u8,
                blue: (value >> 8) as u8,
            },
            PixelFormat::Bgr => Self {
                format,
                red: (value >> 8) as u8,
                green: (value >> 16) as u8,
                blue: (value >> 24) as u8,
            },
        }
    }

    #[must_use]
    #[inline]
    pub const fn new(format: PixelFormat, red: u8, green: u8, blue: u8) -> Self {
        Self {
            format,
            red,
            green,
            blue,
        }
    }

    #[must_use]
    #[inline]
    pub const fn red(&self) -> u8 {
        self.red
    }

    #[must_use]
    #[inline]
    pub const fn green(&self) -> u8 {
        self.green
    }

    #[must_use]
    #[inline]
    pub const fn blue(&self) -> u8 {
        self.blue
    }

    #[must_use]
    #[inline]
    pub const fn format(&self) -> PixelFormat {
        self.format
    }
}
