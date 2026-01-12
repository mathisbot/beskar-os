use crate::{
    error::IoResult,
    io::{File, Read, Seek, SeekFrom, Write},
    mem,
};
use beskar_core::video::{Info, Pixel};
use core::{
    mem::{MaybeUninit, align_of, size_of},
    num::NonZeroU64,
    ops::Range,
};

/// A convenient framebuffer wrapper. It maps an internal buffer and provides
/// simple `flush` semantics to write ranges back to the kernel framebuffer device.
pub struct FrameBuffer {
    info: Info,
    fb_file: File,
    internal_fb: &'static mut [u8],
}

/// A borrowed view over the framebuffer memory.
pub struct FrameView<'a> {
    info: Info,
    stride_bytes: usize,
    bytes: &'a mut [u8],
}

impl FrameBuffer {
    const FB_FILE: &'static str = "/dev/fb";

    #[expect(clippy::missing_panics_doc, reason = "Never panics")]
    /// Open the framebuffer device and map an internal framebuffer.
    ///
    /// # Errors
    ///
    /// Returns an error if the framebuffer device cannot be opened or mapped.
    pub fn open() -> crate::error::Result<Self> {
        let mut fb_file = File::open(Self::FB_FILE)?;

        // Read screen info
        let info = {
            let mut uninit = MaybeUninit::<Info>::uninit();
            let buf = unsafe {
                core::slice::from_raw_parts_mut(uninit.as_mut_ptr().cast::<u8>(), size_of::<Info>())
            };

            fb_file.read_exact(buf)?;

            unsafe { uninit.assume_init() }
        };

        debug_assert_eq!(info.bytes_per_pixel(), 4);

        // Map internal framebuffer
        let internal_fb = {
            let internal_fb_start = mem::mmap(
                u64::from(info.size()),
                Some(NonZeroU64::new(u64::try_from(align_of::<u32>()).unwrap()).unwrap()),
            )?;
            unsafe {
                core::slice::from_raw_parts_mut(
                    internal_fb_start.as_ptr(),
                    usize::try_from(info.size()).unwrap(),
                )
            }
        };
        internal_fb.fill(0);

        Ok(Self {
            info,
            fb_file,
            internal_fb,
        })
    }

    #[must_use]
    #[inline]
    /// Returns the size in bytes of a single framebuffer row.
    pub fn stride_bytes(&self) -> usize {
        usize::from(self.info.stride()) * usize::from(self.info.bytes_per_pixel())
    }

    #[inline]
    #[expect(clippy::missing_panics_doc, reason = "Never panics")]
    /// Flush a range of rows to the kernel framebuffer.
    ///
    /// # Errors
    ///
    /// Returns an error if the write operation fails.
    pub fn flush_rows(&mut self, rows: Range<u16>) -> IoResult<()> {
        let stride_bytes = self.stride_bytes();

        let start_row = usize::from(rows.start.min(self.info.height()));
        let end_row = usize::from(rows.end.min(self.info.height()));

        if start_row >= end_row {
            return Ok(());
        }

        let byte_start = start_row * stride_bytes;
        let byte_end = end_row * stride_bytes;

        self.fb_file
            .seek(SeekFrom::Start(u64::try_from(byte_start).unwrap()))?;
        self.fb_file
            .write_all(&self.internal_fb[byte_start..byte_end])?;
        Ok(())
    }

    #[inline]
    /// Flushes the entire framebuffer.
    ///
    /// # Errors
    ///
    /// Returns an error if the write operation fails.
    pub fn flush_all(&mut self) -> IoResult<()> {
        self.flush_rows(0..self.info.height())
    }

    /// Mutable access to the raw backing buffer.
    #[must_use]
    #[inline]
    pub const fn buffer_mut(&mut self) -> &mut [u8] {
        self.internal_fb
    }

    /// Structured access to the framebuffer.
    #[must_use]
    #[inline]
    pub fn view(&mut self) -> FrameView<'_> {
        FrameView {
            info: self.info,
            stride_bytes: self.stride_bytes(),
            bytes: self.internal_fb,
        }
    }

    /// Returns a view starting at the given row (in pixels).
    #[must_use]
    #[inline]
    #[expect(clippy::missing_panics_doc, reason = "Never panics")]
    pub fn view_from_row(&mut self, start_row: u16) -> FrameView<'_> {
        let stride_bytes = self.stride_bytes();
        let clamped_start = start_row.min(self.info.height());
        let start_byte = usize::from(clamped_start) * stride_bytes;

        let remaining_rows = self.info.height().saturating_sub(clamped_start);
        let view_size = usize::from(remaining_rows) * stride_bytes;

        FrameView {
            info: Info::new(
                view_size.try_into().unwrap(),
                self.info.width(),
                remaining_rows,
                self.info.pixel_format(),
                self.info.stride(),
                self.info.bytes_per_pixel(),
            ),
            stride_bytes,
            bytes: &mut self.internal_fb[start_byte..start_byte + view_size],
        }
    }

    /// Returns layout information of the framebuffer.
    #[must_use]
    #[inline]
    pub const fn info(&self) -> &Info {
        &self.info
    }
}

impl FrameView<'_> {
    #[must_use]
    #[inline]
    pub const fn info(&self) -> Info {
        self.info
    }

    #[must_use]
    #[inline]
    pub const fn bytes_mut(&mut self) -> &mut [u8] {
        self.bytes
    }

    #[must_use]
    #[inline]
    pub fn pixels_mut(&mut self) -> &mut [Pixel] {
        let (prefix, pixels, suffix) = unsafe { self.bytes.align_to_mut::<Pixel>() };
        debug_assert!(prefix.is_empty() && suffix.is_empty());
        pixels
    }

    #[must_use]
    #[inline]
    #[expect(clippy::missing_panics_doc, reason = "Never panics")]
    pub fn sub_rows(&mut self, start_row: u16) -> FrameView<'_> {
        let clamped_start = start_row.min(self.info.height());
        let start_byte = usize::from(clamped_start) * self.stride_bytes;
        let remaining_rows = self.info.height().saturating_sub(clamped_start);
        let view_size = usize::from(remaining_rows) * self.stride_bytes;

        FrameView {
            info: Info::new(
                view_size.try_into().unwrap(),
                self.info.width(),
                remaining_rows,
                self.info.pixel_format(),
                self.info.stride(),
                self.info.bytes_per_pixel(),
            ),
            stride_bytes: self.stride_bytes,
            bytes: &mut self.bytes[start_byte..start_byte + view_size],
        }
    }

    #[must_use]
    #[inline]
    pub fn rows_mut(&mut self, rows: Range<u16>) -> &mut [u8] {
        let start = usize::from(rows.start.min(self.info.height())) * self.stride_bytes;
        let end = usize::from(rows.end.min(self.info.height())) * self.stride_bytes;
        &mut self.bytes[start..end]
    }
}
