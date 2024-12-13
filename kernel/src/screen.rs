// TODO: Refactor

use core::cell::UnsafeCell;

use spin::Once;
use x86_64::{
    structures::paging::{Mapper, Page, PageSize, PageTableFlags, Size4KiB},
    VirtAddr,
};

use crate::{
    mem::{frame_alloc, page_alloc, page_table},
    utils::locks::{McsGuard, McsLock, McsNode},
};

pub mod pixel;
use pixel::PixelFormat;

static SCREEN: Once<Screen> = Once::new();

const MAX_WINDOWS: usize = 16;

pub fn init(raw_buffer: &'static mut [u8], info: ScreenInfo) {
    SCREEN.call_once(|| Screen::new(raw_buffer, info));
}

pub struct Screen {
    raw_buffer: UnsafeCell<&'static mut [u8]>,
    info: ScreenInfo,
    windows: McsLock<[Option<WindowInfo>; MAX_WINDOWS]>,
}

#[derive(Debug, Clone, Copy)]
pub struct ScreenInfo {
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

impl From<bootloader::FrameBufferInfo> for ScreenInfo {
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

// Safety:
// The raw buffer is only accessed "by parts" by the windows.
unsafe impl Send for Screen {}
unsafe impl Sync for Screen {}

// A buffer that can be safely written to by a process.
//
// Once a window is ready to be updated, it can be presented,
// so that its content is displayed on the screen.
pub struct Window {
    raw_buffer: McsLock<&'static mut [u8]>,
    width: usize,
    height: usize,
    index: usize,
    pixel_format: PixelFormat,
    bytes_per_pixel: usize,
}

// Safety:
// All non-Send/Sync fields are read-only.
unsafe impl Send for Window {}
unsafe impl Sync for Window {}

impl Screen {
    pub const fn new(raw_buffer: &'static mut [u8], info: ScreenInfo) -> Self {
        Self {
            raw_buffer: UnsafeCell::new(raw_buffer),
            info,
            windows: McsLock::new([const { None }; MAX_WINDOWS]),
        }
    }

    pub fn create_window(&self, x: usize, y: usize, width: usize, height: usize) -> Option<Window> {
        assert!(
            width + x <= self.info.width && height + y <= self.info.height,
            "Window has to fit in the screen"
        );

        let node = McsNode::new();
        let mut windows = self.windows.lock(&node);

        assert!(
            windows.iter().all(|window| {
                window.as_ref().is_none_or(|window| {
                    x >= window.x + window.width
                        || x + width <= window.x
                        || y >= window.y + window.height
                        || y + height <= window.y
                })
            }),
            "Window cannot overlap with existing windows"
        );

        for (index, window) in windows.iter_mut().enumerate() {
            if window.is_none() {
                // Friendly reminder that there should be no heap allocation here

                let pages = page_alloc::with_page_allocator(|page_allocator| {
                    page_allocator
                        .allocate_pages::<Size4KiB>(
                            u64::try_from(
                                (width * height * self.info.bytes_per_pixel)
                                    .div_ceil(usize::try_from(Size4KiB::SIZE).unwrap()),
                            )
                            .unwrap(),
                        )
                        .unwrap()
                });

                frame_alloc::with_frame_allocator(|frame_allocator| {
                    frame_allocator.map_pages(
                        pages,
                        PageTableFlags::PRESENT
                            | PageTableFlags::WRITABLE
                            | PageTableFlags::NO_EXECUTE,
                    );
                });

                // Safety:
                // The page is mapped to a frame and we made sure the window fits in a page.
                let raw_buffer = unsafe {
                    core::slice::from_raw_parts_mut(
                        pages.start.start_address().as_mut_ptr::<u8>(),
                        width * height * self.info.bytes_per_pixel,
                    )
                };

                *window = Some(WindowInfo {
                    x,
                    y,
                    width,
                    height,
                });

                return Some(Window {
                    raw_buffer: McsLock::new(raw_buffer),
                    width,
                    height,
                    index,
                    pixel_format: self.info.pixel_format,
                    bytes_per_pixel: self.info.bytes_per_pixel,
                });
            }
        }

        None
    }

    // FIXME: If window's raw buffer is locked, there's a deadlocks.
    /// Present a window on the screen.
    ///
    /// The window's buffer must NOT be locked when calling this function.
    pub fn present_window(&self, window: &Window) {
        assert!(window.index < MAX_WINDOWS, "Invalid window index");

        let node = McsNode::new();
        let windows = self.windows.lock(&node);
        let Some(&window_info) = &windows[window.index].as_ref() else {
            panic!("Window not found");
        };
        // Row by row copy
        let line_length = window.width * window.bytes_per_pixel;
        for h in 0..window.height {
            let offset_in_screen = (
                // Position in window
                (window_info.y + h) * self.info.stride
                + window_info.x
            )
            // Convert to bytes
            * self.info.bytes_per_pixel;

            let offset_in_window = h * window.width * window.bytes_per_pixel;

            // Safety:
            // The window is locked and the part of the buffer accessed is reserved for the window.
            let raw_buffer = unsafe { &mut *self.raw_buffer.get() };

            // Copy the window's buffer to the screen's buffer
            raw_buffer[offset_in_screen..offset_in_screen + line_length].copy_from_slice(
                &window.raw_buffer.lock(&node)[offset_in_window..offset_in_window + line_length],
            );
        }
        // windows should be locked until the current window is presented
        drop(windows);
    }

    // TODO: Reference screen in `Window` and automatically destroy window on drop
    /// Destroy a window.
    ///
    /// ## Safety
    ///
    /// The window must not be used after this function is called.
    pub unsafe fn destroy_window(&self, window: &Window) {
        assert!(window.index < MAX_WINDOWS, "Invalid window index");

        let node = McsNode::new();
        let mut windows = self.windows.lock(&node);

        assert!(windows[window.index].is_some(), "Window not found");

        // TODO: Should the window be cleared before being destroyed?
        windows[window.index] = None;
    }
}

impl Window {
    #[must_use]
    #[inline]
    pub fn buffer_mut<'s, 'node>(
        &'s self,
        node: &'node McsNode,
    ) -> McsGuard<'node, 's, &'static mut [u8]> {
        self.raw_buffer.lock(node)
    }

    #[must_use]
    #[inline]
    pub const fn width(&self) -> usize {
        self.width
    }

    #[must_use]
    #[inline]
    pub const fn height(&self) -> usize {
        self.height
    }

    #[must_use]
    #[inline]
    pub const fn pixel_format(&self) -> PixelFormat {
        self.pixel_format
    }

    #[must_use]
    #[inline]
    pub const fn bytes_per_pixel(&self) -> usize {
        self.bytes_per_pixel
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        let (start_vaddr, end_vaddr) = self.raw_buffer.with_locked(|raw_buffer| {
            let start_vaddr = VirtAddr::new(raw_buffer.as_ptr() as u64);
            let end_vaddr = start_vaddr + u64::try_from(size_of_val(*raw_buffer)).unwrap();

            (start_vaddr, end_vaddr)
        });

        let page_start = Page::<Size4KiB>::containing_address(start_vaddr);
        let page_end = Page::<Size4KiB>::containing_address(end_vaddr);

        for page in Page::range_inclusive(page_start, page_end) {
            let frame = page_table::with_page_table(|page_table| {
                let (frame, tlb) = page_table.unmap(page).unwrap();
                tlb.flush();
                frame
            });

            frame_alloc::with_frame_allocator(|frame_allocator| {
                frame_allocator.free(frame);
            });
        }

        page_alloc::with_page_allocator(|page_allocator| {
            page_allocator.free_pages(Page::range_inclusive(page_start, page_end));
        });
    }
}

#[derive(Debug, Clone, Copy)]
struct WindowInfo {
    x: usize,
    y: usize,
    width: usize,
    height: usize,
}

pub fn get_screen() -> &'static Screen {
    SCREEN.get().expect("Screen not initialized")
}
