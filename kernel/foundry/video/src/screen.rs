use beskar_core::{
    storage::{BlockDeviceError, KernelDevice},
    video::{FrameBuffer, Info, Pixel, PixelComponents},
};
use hyperdrive::locks::mcs::MUMcsLock;

static SCREEN: MUMcsLock<Screen> = MUMcsLock::uninit();

pub fn init(frame_buffer: &'static mut FrameBuffer) {
    let info = frame_buffer.info();
    assert_eq!(
        info.bytes_per_pixel(),
        4,
        "Only 32-bit pixels are supported"
    );
    let screen = Screen::new(frame_buffer.buffer_mut(), info);

    SCREEN.init(screen);

    crate::info!(
        "Screen initialized with resolution {}x{}",
        info.width(),
        info.height()
    );
}

pub struct Screen<'a> {
    raw_buffer: &'a mut [Pixel],
    info: Info,
}

impl<'a> Screen<'a> {
    #[must_use]
    #[inline]
    pub fn new(raw_buffer: &'a mut [u8], info: Info) -> Self {
        assert_eq!(size_of::<Pixel>(), usize::from(info.bytes_per_pixel()));

        // Safety: A `Pixel` is a `u32` and it is valid to pack 4 `u8`s into a `u32`.
        let (start_u8, pixel_buffer, end_u8) = unsafe { raw_buffer.align_to_mut::<Pixel>() };

        assert!(
            start_u8.is_empty() && end_u8.is_empty(),
            "Buffer is not aligned to Pixel"
        );

        Self {
            raw_buffer: pixel_buffer,
            info,
        }
    }
}

impl Screen<'_> {
    #[must_use]
    #[inline]
    pub const fn info(&self) -> Info {
        self.info
    }

    #[must_use]
    #[inline]
    /// Returns a reference to the raw buffer.
    pub const fn buffer_mut(&mut self) -> &mut [Pixel] {
        self.raw_buffer
    }

    /// Clears the screen with the given pixel.
    pub fn clear(&mut self, pixel: Pixel) {
        self.raw_buffer.fill(pixel);
    }
}

#[inline]
pub fn with_screen<R, F: FnOnce(&mut Screen) -> R>(f: F) -> R {
    SCREEN.with_locked(f)
}

#[derive(Debug, Default, Clone)]
pub struct ScreenDevice;

impl ScreenDevice {
    #[must_use]
    #[inline]
    pub const fn new() -> Self {
        Self
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(align(4))]
struct PixelCompArr(PixelComponents);

impl KernelDevice for ScreenDevice {
    fn read(&mut self, dst: &mut [u8], _offset: usize) -> Result<(), BlockDeviceError> {
        if dst.is_empty() {
            return Ok(());
        }

        if dst.len() == size_of::<Info>() {
            // FIXME: If https://github.com/rust-lang/libs-team/issues/588 is implemented, use it.
            #[expect(clippy::cast_ptr_alignment, reason = "Alignment is checked manually")]
            let pixel_ptr = dst.as_mut_ptr().cast::<Info>();
            if !pixel_ptr.is_aligned() {
                return Err(BlockDeviceError::UnalignedAccess);
            }
            unsafe {
                pixel_ptr.write(with_screen(|screen| screen.info()));
            }

            return Ok(());
        }

        Err(BlockDeviceError::Unsupported)
    }

    fn write(&mut self, src: &[u8], offset: usize) -> Result<(), BlockDeviceError> {
        let (prefix, src, suffix) = unsafe { src.align_to::<PixelCompArr>() };

        if !prefix.is_empty() || !suffix.is_empty() {
            return Err(BlockDeviceError::UnalignedAccess);
        }

        with_screen(|screen| {
            if src.len() + offset > screen.info().size().try_into().unwrap() {
                return Err(BlockDeviceError::OutOfBounds);
            }

            let pixel_format = screen.info().pixel_format();
            let screen_buffer = screen.buffer_mut();

            for (&pc, d) in src.iter().zip(screen_buffer[offset..].iter_mut()) {
                let pixel = Pixel::from_format(pixel_format, pc.0);
                *d = pixel;
            }

            Ok(())
        })
    }

    fn on_open(&mut self) {
        super::log::set_screen_logging(false);
        with_screen(|screen| {
            screen.clear(Pixel::from_format(
                screen.info().pixel_format(),
                PixelComponents::BLACK,
            ));
        });
    }

    fn on_close(&mut self) {
        with_screen(|screen| {
            screen.clear(Pixel::from_format(
                screen.info().pixel_format(),
                PixelComponents::BLACK,
            ));
        });
        super::log::with_fb_writer(beskar_core::video::writer::FramebufferWriter::soft_clear);
        super::log::set_screen_logging(true);
    }
}
