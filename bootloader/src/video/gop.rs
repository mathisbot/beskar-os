//! Handles the Graphical Output Protocol (GOP) provided by the UEFI firmware.

use beskar_core::arch::PhysAddr;
use beskar_core::video::{Info, PixelBitmask, PixelFormat};
use uefi::{
    boot,
    proto::console::gop::{self, GraphicsOutput},
};

use super::PhysicalFrameBuffer;

#[must_use]
/// Initializes the GOP and returns the (physical) framebuffer.
pub fn init() -> PhysicalFrameBuffer {
    let mut gop = {
        // Starting from UEFI 2.0, locating GOP cannot fail.
        let gop_handle = boot::get_handle_for_protocol::<GraphicsOutput>().unwrap();
        boot::open_protocol_exclusive::<GraphicsOutput>(gop_handle).unwrap()
    };

    let best_mode = gop
        .modes()
        .max_by(|a, b| {
            // BltOnly pixel format is not supported
            // as it won't be available for the kernel.
            if a.info().pixel_format() == gop::PixelFormat::BltOnly {
                return core::cmp::Ordering::Less;
            }

            let res_a = a.info().resolution();
            let res_b = b.info().resolution();

            match res_a.0.cmp(&res_b.0) {
                core::cmp::Ordering::Equal => res_a.1.cmp(&res_b.1),
                other => other,
            }
        })
        .unwrap();

    let mode_info = best_mode.info();

    let pixel_format = match mode_info.pixel_format() {
        gop::PixelFormat::Rgb => PixelFormat::Rgb,
        gop::PixelFormat::Bgr => PixelFormat::Bgr,
        gop::PixelFormat::Bitmask => {
            let info_bm = mode_info.pixel_bitmask().unwrap();
            let bitmask = PixelBitmask {
                red: info_bm.red,
                green: info_bm.green,
                blue: info_bm.blue,
            };
            PixelFormat::Bitmask(bitmask)
        }
        gop::PixelFormat::BltOnly => {
            panic!("BltOnly pixel format is not supported");
        }
    };

    gop.set_mode(&best_mode).unwrap();

    let mut gop_fb = gop.frame_buffer();

    // Safety:
    // The framebuffer address and buffer length are valid because they are derived
    // from the GOP-provided framebuffer, which guarantees their correctness FOR NOW.
    //
    // The reasons we cannot use the FrameBuffer struct directly is because it mutably borrows `gop`
    // to make sure the mode doesn't get changed, resulting in immediate UB for the framebuffer.
    // Here, I guarantee the GOP mode will not be changed ever again!
    let fb_slice = unsafe { core::slice::from_raw_parts_mut(gop_fb.as_mut_ptr(), gop_fb.size()) };

    PhysicalFrameBuffer {
        start_addr: PhysAddr::new(fb_slice.as_mut_ptr() as u64),
        info: Info::new(
            gop_fb.size().try_into().unwrap(),
            mode_info.resolution().0.try_into().unwrap(),
            mode_info.resolution().1.try_into().unwrap(),
            pixel_format,
            mode_info.stride().try_into().unwrap(),
            4,
        ),
    }
}
