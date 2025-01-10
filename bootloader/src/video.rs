//! Responsible for initializiing graphical output

use hyperdrive::locks::mcs::MUMcsLock;
use uefi::proto::console::gop::PixelBitmask;
use x86_64::{PhysAddr, VirtAddr};

pub mod gop;

static PHYSICAL_FB: MUMcsLock<PhysicalFrameBuffer> = MUMcsLock::uninit();

pub fn init() {
    let p_fb = gop::init();
    PHYSICAL_FB.init(p_fb);
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

#[derive(Debug)]
/// Represents a framebuffer with its physical address.
pub struct PhysicalFrameBuffer {
    start_addr: PhysAddr,
    info: FrameBufferInfo,
}

impl PhysicalFrameBuffer {
    #[must_use]
    #[inline]
    pub const fn start_addr(&self) -> PhysAddr {
        self.start_addr
    }

    #[must_use]
    #[inline]
    pub const fn info(&self) -> FrameBufferInfo {
        self.info
    }

    #[must_use]
    pub const fn start_addr_as_virtual(&self) -> VirtAddr {
        VirtAddr::new(self.start_addr.as_u64())
    }

    #[must_use]
    /// Access the raw bytes of the framebuffer as a mutable slice.
    pub const fn buffer_mut(&mut self) -> &mut [u8] {
        unsafe {
            core::slice::from_raw_parts_mut(
                self.start_addr_as_virtual().as_mut_ptr::<u8>(),
                self.info().size,
            )
        }
    }

    #[must_use]
    #[inline]
    /// Converts the physical framebuffer to a (virtual) framebuffer.
    ///
    /// ## Safety
    ///
    /// The provided framebuffer must only be used to transfer the framebuffer to the kernel.
    pub const unsafe fn to_framebuffer(&self, vaddr: VirtAddr) -> FrameBuffer {
        FrameBuffer {
            buffer_start: vaddr,
            info: self.info,
        }
    }
}

/// Represents a frambuffer.
///
/// This is the struct that is sent to the kernel.
#[derive(Debug)]
pub struct FrameBuffer {
    buffer_start: VirtAddr,
    info: FrameBufferInfo,
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
    size: usize,
    /// The width in pixels.
    width: usize,
    /// The height in pixels.
    height: usize,
    /// The color format of each pixel.
    pixel_format: PixelFormat,
    /// The number of bytes per pixel.
    bytes_per_pixel: usize,
    /// Number of "virtual" pixels between the start of a line and the start of the next.
    ///
    /// The stride must be used to compute the start address of a next line as some framebuffers
    /// use additional padding at the end of a line.
    stride: usize,
}

impl FrameBufferInfo {
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

#[inline]
pub fn clear_screen() {
    with_physical_framebuffer(|fb| {
        fb.buffer_mut().fill(0);
    });
}

pub fn with_physical_framebuffer<T, F: FnOnce(&mut PhysicalFrameBuffer) -> T>(f: F) -> T {
    PHYSICAL_FB.with_locked(f)
}

// #[must_use]
// /// Initializes the framebuffer logger for use by the bootloader.
// ///
// /// The framebuffer is used to display log messages after boot services are exited.
// /// This framebuffer will also be passed to the kernel for further use.
// ///
// /// ## Note
// ///
// /// The bootloader currently freezes the screen here if the screen only support `BltOnly` pixel format.
// fn create_framebuffer_logger() -> bootloader::PhysicalFrameBuffer {
//     let logger = bootloader::logging::init(framebuffer_slice, framebuffer_info);
//     log::set_logger(logger).expect("Failed to set logger");
//     log::set_max_level(if cfg!(debug_assertions) {
//         log::LevelFilter::Trace
//     } else {
//         log::LevelFilter::Info
//     });

//     trace!(
//         "Framebuffer initialized at {}x{}",
//         framebuffer_info.width, framebuffer_info.height
//     );

//     // Safety:
//     // The framebuffer address and information are valid because they are derived
//     // from the GOP-provided framebuffer, which guarantees their correctness.
//     unsafe {
//         bootloader::PhysicalFrameBuffer::new(
//             PhysAddr::new(framebuffer.as_mut_ptr() as u64),
//             framebuffer_info,
//         )
//     }
// }
