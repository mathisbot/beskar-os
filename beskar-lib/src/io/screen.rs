use core::mem::{MaybeUninit, align_of, size_of};
use core::num::NonZeroU64;
use core::ops::Range;

use crate::error::{IoError, IoErrorKind, IoResult};
use crate::io::SeekFrom;
use crate::io::{File, Read, Seek, Write};
use crate::mem;
use beskar_core::video::Info;

/// A convenient framebuffer wrapper. It maps an internal buffer and provides
/// simple `flush` semantics to write ranges back to the kernel framebuffer device.
pub struct FrameBuffer {
    info: Info,
    fb_file: File,
    internal_fb: &'static mut [u8],
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
        // Read screen info
        let mut info_uninit = MaybeUninit::<Info>::uninit();
        let info_buf = unsafe {
            core::slice::from_raw_parts_mut(
                info_uninit.as_mut_ptr().cast::<u8>(),
                size_of::<Info>(),
            )
        };

        let mut fb_file = File::open(Self::FB_FILE)?;
        fb_file.read_exact(info_buf)?;
        let info = unsafe { info_uninit.assume_init() };

        // Map internal framebuffer
        let internal_fb_start = mem::mmap(
            u64::from(info.size()),
            Some(NonZeroU64::new(u64::try_from(align_of::<u32>()).unwrap()).unwrap()),
        )
        .map_err(|_| IoError::new(IoErrorKind::Other))?;
        let internal_fb = unsafe {
            core::slice::from_raw_parts_mut(
                internal_fb_start.as_ptr(),
                usize::try_from(info.size()).unwrap(),
            )
        };

        // Clear the internal framebuffer
        {
            let (prefix, large, suffix) = unsafe { internal_fb.align_to_mut::<u64>() };
            prefix.fill(0);
            large.fill(0);
            suffix.fill(0);
        }

        Ok(Self {
            info,
            fb_file,
            internal_fb,
        })
    }

    #[expect(clippy::missing_panics_doc, reason = "Never panics")]
    /// Flush a range of rows (or the whole buffer if `rows` is `None`) to the kernel framebuffer.
    ///
    /// # Errors
    ///
    /// Returns an error if writing to the framebuffer device fails.
    pub fn flush(&mut self, rows: Option<&Range<u16>>) -> IoResult<()> {
        let stride = usize::from(self.info.stride());
        let max_row = usize::from(self.info.height());
        let bpp = usize::from(self.info.bytes_per_pixel());

        let offset_in_screen = rows
            .as_ref()
            .map_or(0, |r| usize::from(r.start) * stride)
            .min(max_row * stride);

        let offset = offset_in_screen * bpp;

        let end = rows
            .as_ref()
            .map_or_else(
                || usize::try_from(self.info.size()).unwrap(),
                |r| usize::from(r.end) * stride * bpp,
            )
            .min(max_row * stride * bpp);

        self.fb_file
            .seek(SeekFrom::Start(u64::try_from(offset_in_screen).unwrap()))?;
        self.fb_file.write_all(&self.internal_fb[offset..end])?;
        Ok(())
    }

    /// Mutable access to the internal buffer.
    #[must_use]
    #[inline]
    pub const fn buffer_mut(&mut self) -> &mut [u8] {
        self.internal_fb
    }

    /// Returns a reference the stored `Info`.
    #[must_use]
    #[inline]
    pub const fn info(&self) -> &Info {
        &self.info
    }
}
