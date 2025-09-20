use beskar_core::video::Info;
use core::{mem::MaybeUninit, num::NonZeroU64, ops::Range};
use hyperdrive::{locks::mcs::MUMcsLock, once::Once};

static SCREEN: MUMcsLock<Screen> = MUMcsLock::uninit();
static SCREEN_INFO: Once<Info> = Once::uninit();

const FB_FILE: &str = "/dev/fb";

pub fn init() {
    SCREEN.init(Screen::new());
    SCREEN_INFO.call_once(|| SCREEN.with_locked(|screen| screen.info));
}

fn read_screen_info() -> Info {
    let mut info = MaybeUninit::<Info>::uninit();

    let info_buff = unsafe {
        core::slice::from_raw_parts_mut(info.as_mut_ptr().cast::<u8>(), size_of::<Info>())
    };

    let fb_file = beskar_lib::io::File::open(FB_FILE).unwrap();

    fb_file.read(info_buff, 0).unwrap();

    // Safety: We just initialized the memory with a valid Info struct.
    unsafe { info.assume_init() }
}

pub struct Screen {
    info: Info,
    fb_file: beskar_lib::io::File,
    internal_fb: &'static mut [u8],
}

impl Default for Screen {
    fn default() -> Self {
        Self::new()
    }
}

impl Screen {
    #[must_use]
    /// # Panics
    ///
    /// Panics if the framebuffer file cannot be opened.
    pub fn new() -> Self {
        let info = read_screen_info();

        let fb_file = beskar_lib::io::File::open(FB_FILE).unwrap();

        let internal_fb_start = beskar_lib::mem::mmap(
            u64::from(info.size()),
            Some(NonZeroU64::new(align_of::<u32>().try_into().unwrap()).unwrap()),
        );
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

        Self {
            info,
            fb_file,
            internal_fb,
        }
    }

    #[inline]
    #[expect(clippy::missing_panics_doc, reason = "Never panics")]
    #[expect(clippy::needless_pass_by_value, reason = "This is ugly otherwise")]
    pub fn flush(&self, rows: Option<Range<u16>>) {
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
            .write(&self.internal_fb[offset..end], offset_in_screen)
            .unwrap();
    }

    #[must_use]
    #[inline]
    pub const fn buffer_mut(&mut self) -> &mut [u8] {
        self.internal_fb
    }
}

/// Returns the screen info.
///
/// # Panics
///
/// Panics if the screen info has not been initialized yet.
pub fn screen_info() -> &'static Info {
    SCREEN_INFO.get().unwrap()
}

pub fn with_screen<R, F: FnOnce(&mut Screen) -> R>(f: F) -> R {
    SCREEN.with_locked(f)
}
