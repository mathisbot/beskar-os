//! Core data structures for video subsystem.
use crate::arch::VirtAddr;

pub mod writer;

/// Supported pixel formats
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, num_enum::TryFromPrimitive, num_enum::IntoPrimitive,
)]
#[non_exhaustive]
#[repr(u8)]
pub enum PixelFormat {
    /// 32-bit ARGB with 8 bits per channel
    Argb8888,
    /// 32-bit XRGB with 8 bits per channel
    Xrgb8888,
    /// 32-bit BGR with 8 bits per channel
    Bgr8888,
}

impl PixelFormat {
    /// Returns the size in bytes for one pixel
    #[inline]
    #[must_use]
    pub const fn bytes_per_pixel(self) -> u8 {
        match self {
            Self::Argb8888 | Self::Xrgb8888 | Self::Bgr8888 => 4,
        }
    }
}

/// A point in 2D space
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct Point {
    pub x: u16,
    pub y: u16,
}

impl Point {
    /// Create a new point
    #[must_use]
    pub const fn new(x: u16, y: u16) -> Self {
        Self { x, y }
    }
}

/// A rectangle defined by top-left corner and dimensions
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct Rect {
    pub top_left: Point,
    pub width: u16,
    pub height: u16,
}

impl Rect {
    /// Create a new rectangle
    #[must_use]
    pub const fn new(x: u16, y: u16, width: u16, height: u16) -> Self {
        Self {
            top_left: Point::new(x, y),
            width,
            height,
        }
    }

    /// Merge this rect with another
    #[must_use]
    pub fn merge(self, other: Self) -> Self {
        let x1 = self.top_left.x.min(other.top_left.x);
        let y1 = self.top_left.y.min(other.top_left.y);
        let x2 = (self.top_left.x + self.width).max(other.top_left.x + other.width);
        let y2 = (self.top_left.y + self.height).max(other.top_left.y + other.height);

        Self {
            top_left: Point::new(x1, y1),
            width: x2 - x1,
            height: y2 - y1,
        }
    }

    /// Check if this rect contains another
    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.top_left.x <= other.top_left.x
            && self.top_left.y <= other.top_left.y
            && self.top_left.x + self.width >= other.top_left.x + other.width
            && self.top_left.y + self.height >= other.top_left.y + other.height
    }

    /// Clip rect to bounds
    #[must_use]
    pub fn clip_to_bounds(self, max_width: u16, max_height: u16) -> Option<Self> {
        let x = self.top_left.x;
        let y = self.top_left.y;

        if x >= max_width || y >= max_height {
            return None;
        }

        let width = self.width.min(max_width - x);
        let height = self.height.min(max_height - y);

        if width == 0 || height == 0 {
            None
        } else {
            Some(Self {
                top_left: Point::new(x, y),
                width,
                height,
            })
        }
    }
}

/// Framebuffer description
#[derive(Debug, Clone)]
pub struct FramebufferConfig {
    /// Framebuffer info
    info: Info,
    /// Framebuffer base pointer
    buffer: VirtAddr,
}

impl FramebufferConfig {
    #[must_use]
    #[inline]
    pub const fn new(
        width: u16,
        height: u16,
        stride: u16,
        format: PixelFormat,
        buffer: VirtAddr,
    ) -> Self {
        let info = Info::new(width, height, format, stride);
        Self { info, buffer }
    }

    /// Get the total size of the framebuffer in bytes
    #[must_use]
    #[inline]
    pub const fn size(&self) -> u32 {
        self.info.size()
    }

    #[must_use]
    #[inline]
    /// The framebuffer rect covering the entire area
    pub const fn rect(&self) -> Rect {
        Rect {
            top_left: Point::new(0, 0),
            width: self.info.width(),
            height: self.info.height(),
        }
    }

    #[must_use]
    #[inline]
    pub const fn info(&self) -> Info {
        self.info
    }

    #[must_use]
    #[inline]
    pub const fn buffer_ptr(&self) -> *mut u8 {
        self.buffer.as_mut_ptr()
    }
}

/// Surface ID for tracking offscreen surfaces
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SurfaceId(pub u32);

/// An offscreen rendering surface
///
/// Surfaces hold a reference to externally-managed buffer memory (e.g., from mmap).
/// The kernel is responsible for allocating and providing the buffer pointer.
#[derive(Debug)]
pub struct Surface {
    id: SurfaceId,
    x: u16,
    y: u16,
    width: u16,
    height: u16,
    format: PixelFormat,
    /// Buffer pointer
    buffer: VirtAddr,
    /// Dirty region that needs redraw
    dirty: Option<Rect>,
}

impl Surface {
    #[must_use]
    /// Create a new surface with an externally-provided buffer
    ///
    /// # Safety
    ///
    /// Caller must ensure:
    /// - `buffer` points to valid memory
    /// - Buffer remains valid for the lifetime of this Surface
    /// - Buffer has at least `width * height * format.bytes_per_pixel()` bytes
    pub unsafe fn new_with_buffer(
        id: SurfaceId,
        x: u16,
        y: u16,
        width: u16,
        height: u16,
        format: PixelFormat,
        buffer: *mut u8,
    ) -> Self {
        Self {
            id,
            x,
            y,
            width,
            height,
            format,
            buffer: VirtAddr::from_ptr(buffer),
            dirty: None,
        }
    }

    #[must_use]
    #[inline]
    pub const fn id(&self) -> SurfaceId {
        self.id
    }

    #[must_use]
    #[inline]
    pub const fn x(&self) -> u16 {
        self.x
    }

    #[must_use]
    #[inline]
    pub const fn y(&self) -> u16 {
        self.y
    }

    #[must_use]
    #[inline]
    pub const fn width(&self) -> u16 {
        self.width
    }

    #[must_use]
    #[inline]
    pub const fn height(&self) -> u16 {
        self.height
    }

    #[must_use]
    #[inline]
    pub const fn format(&self) -> PixelFormat {
        self.format
    }

    #[must_use]
    #[inline]
    pub const fn buffer_ptr(&self) -> *mut u8 {
        self.buffer.as_mut_ptr()
    }

    #[must_use]
    #[inline]
    pub const fn dirty_rect(&self) -> Option<Rect> {
        self.dirty
    }

    /// Mark a region as dirty
    pub fn mark_dirty(&mut self, rect: Rect) {
        let Some(rect) = rect.clip_to_bounds(self.width, self.height) else {
            return;
        };

        self.dirty = Some(self.dirty.map_or(rect, |existing| existing.merge(rect)));
    }

    /// Mark entire surface as dirty
    #[inline]
    pub const fn mark_all_dirty(&mut self) {
        self.dirty = Some(Rect::new(0, 0, self.width, self.height));
    }

    /// Clear the dirty flag
    #[inline]
    pub const fn clear_dirty(&mut self) {
        self.dirty = None;
    }

    #[must_use]
    #[inline]
    /// Take the current dirty rect and clear it
    pub const fn take_dirty(&mut self) -> Option<Rect> {
        self.dirty.take()
    }

    /// Get the stride in bytes for this surface
    #[must_use]
    #[inline]
    pub const fn stride_bytes(&self) -> u16 {
        self.width * (self.format.bytes_per_pixel() as u16)
    }

    #[must_use]
    #[inline]
    pub const fn size(&self) -> usize {
        (self.width as usize) * (self.height as usize) * (self.format.bytes_per_pixel() as usize)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct Pixel(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct PixelComponents {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: u8,
}
crate::static_assert!(size_of::<PixelComponents>() == size_of::<Pixel>());

impl PixelComponents {
    pub const WHITE: Self = Self {
        red: 0xFF,
        green: 0xFF,
        blue: 0xFF,
        alpha: 0xFF,
    };
    pub const BLACK: Self = Self {
        red: 0x00,
        green: 0x00,
        blue: 0x00,
        alpha: 0xFF,
    };

    pub const RED: Self = Self {
        red: 0xFF,
        green: 0x00,
        blue: 0x00,
        alpha: 0xFF,
    };
    pub const GREEN: Self = Self {
        red: 0x00,
        green: 0xFF,
        blue: 0x00,
        alpha: 0xFF,
    };
    pub const BLUE: Self = Self {
        red: 0x00,
        green: 0x00,
        blue: 0xFF,
        alpha: 0xFF,
    };

    pub const CYAN: Self = Self {
        red: 0x00,
        green: 0xFF,
        blue: 0xFF,
        alpha: 0xFF,
    };
    pub const MAGENTA: Self = Self {
        red: 0xFF,
        green: 0x00,
        blue: 0xFF,
        alpha: 0xFF,
    };
    pub const YELLOW: Self = Self {
        red: 0xFF,
        green: 0xFF,
        blue: 0x00,
        alpha: 0xFF,
    };

    pub const ORANGE: Self = Self {
        red: 0xFF,
        green: 0xA5,
        blue: 0x00,
        alpha: 0xFF,
    };

    #[must_use]
    #[inline]
    pub const fn new(red: u8, green: u8, blue: u8, alpha: u8) -> Self {
        Self {
            red,
            green,
            blue,
            alpha,
        }
    }
}

impl core::ops::Add<Self> for PixelComponents {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            red: self.red.saturating_add(rhs.red),
            green: self.green.saturating_add(rhs.green),
            blue: self.blue.saturating_add(rhs.blue),
            alpha: self.alpha.saturating_add(rhs.alpha),
        }
    }
}

impl core::ops::Mul for PixelComponents {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        #[inline]
        fn mul_chan(a: u8, b: u8) -> u8 {
            let x = u16::from(a) * u16::from(b) + 128;
            ((x + (x >> 8)) >> 8) as u8
        }

        Self {
            red: mul_chan(self.red, rhs.red),
            green: mul_chan(self.green, rhs.green),
            blue: mul_chan(self.blue, rhs.blue),
            alpha: mul_chan(self.alpha, rhs.alpha),
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
            PixelFormat::Argb8888 | PixelFormat::Xrgb8888 => Self::new_rgba(components),
            PixelFormat::Bgr8888 => Self::new_bgra(components),
        }
    }

    #[must_use]
    #[inline]
    pub fn new_rgba(components: PixelComponents) -> Self {
        Self(
            ((u32::from(components.alpha)) << 24)
                | ((u32::from(components.blue)) << 16)
                | ((u32::from(components.green)) << 8)
                | u32::from(components.red),
        )
    }

    #[must_use]
    #[inline]
    pub fn new_bgra(components: PixelComponents) -> Self {
        Self(
            ((u32::from(components.alpha)) << 24)
                | ((u32::from(components.red)) << 16)
                | ((u32::from(components.green)) << 8)
                | u32::from(components.blue),
        )
    }

    #[must_use]
    #[inline]
    pub fn components_by_format(self, format: PixelFormat) -> PixelComponents {
        match format {
            PixelFormat::Argb8888 | PixelFormat::Xrgb8888 => self.components_rgba(),
            PixelFormat::Bgr8888 => self.components_bgra(),
        }
    }

    #[must_use]
    #[inline]
    pub fn components_bgra(self) -> PixelComponents {
        let red = u8::try_from((self.0 >> 16) & 0xFF).unwrap();
        let green = u8::try_from((self.0 >> 8) & 0xFF).unwrap();
        let blue = u8::try_from(self.0 & 0xFF).unwrap();
        let alpha = u8::try_from((self.0 >> 24) & 0xFF).unwrap();
        PixelComponents {
            red,
            green,
            blue,
            alpha,
        }
    }

    #[must_use]
    #[inline]
    pub fn components_rgba(self) -> PixelComponents {
        let blue = u8::try_from((self.0 >> 16) & 0xFF).unwrap();
        let green = u8::try_from((self.0 >> 8) & 0xFF).unwrap();
        let red = u8::try_from(self.0 & 0xFF).unwrap();
        let alpha = u8::try_from((self.0 >> 24) & 0xFF).unwrap();
        PixelComponents {
            red,
            green,
            blue,
            alpha,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Info {
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
}

impl Info {
    #[must_use]
    #[inline]
    pub const fn new(width: u16, height: u16, pixel_format: PixelFormat, stride: u16) -> Self {
        Self {
            width,
            height,
            pixel_format,
            stride,
        }
    }

    #[must_use]
    #[inline]
    /// The total size in bytes.
    pub const fn size(&self) -> u32 {
        self.stride as u32 * self.height as u32 * self.pixel_format.bytes_per_pixel() as u32
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
        self.pixel_format().bytes_per_pixel()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dirty_rect_merge() {
        let rect1 = Rect::new(0, 0, 100, 100);
        let rect2 = Rect::new(50, 50, 100, 100);
        let merged = rect1.merge(rect2);

        assert_eq!(merged.top_left.x, 0);
        assert_eq!(merged.top_left.y, 0);
        assert_eq!(merged.width, 150);
        assert_eq!(merged.height, 150);
    }

    #[test]
    fn test_dirty_rect_clip() {
        let rect = Rect::new(50, 50, 100, 100);
        let clipped = rect.clip_to_bounds(120, 120).unwrap();

        assert_eq!(clipped.width, 70);
        assert_eq!(clipped.height, 70);
    }

    #[test]
    fn test_surface_creation() {
        // Create a small test buffer on the stack
        let mut buffer = [0_u8; 640 * 480 * 4];
        let surface = unsafe {
            Surface::new_with_buffer(
                SurfaceId(1),
                0,
                0,
                640,
                480,
                PixelFormat::Argb8888,
                buffer.as_mut_ptr(),
            )
        };
        assert_eq!(surface.width, 640);
        assert_eq!(surface.height, 480);
        assert_eq!(surface.stride_bytes(), 640 * 4);
    }

    #[test]
    fn test_pixel_components_ops() {
        let pixel_components_1 = PixelComponents {
            red: 0x10,
            green: 0x20,
            blue: 0x30,
            alpha: 0xFF,
        };
        let pixel_components_2 = PixelComponents {
            red: 0x40,
            green: 0x50,
            blue: 0x60,
            alpha: 0xFF,
        };

        let sum = pixel_components_1 + pixel_components_2;
        assert_eq!(
            sum,
            PixelComponents {
                red: 0x50,
                green: 0x70,
                blue: 0x90,
                alpha: 0xFF,
            }
        );

        let mul = pixel_components_1 * pixel_components_2;
        assert_eq!(
            mul,
            PixelComponents {
                red: 0x04,
                green: 0x0A,
                blue: 0x12,
                alpha: 0xFF,
            }
        );
    }
}
