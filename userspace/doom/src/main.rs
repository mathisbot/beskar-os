#![no_std]
#![forbid(unsafe_op_in_unsafe_fn)]
#![warn(clippy::pedantic, clippy::nursery)]
#![no_main]
use core::ffi::c_char;

#[link(name = "puredoom", kind = "static")]
unsafe extern "C" {
    unsafe fn doom_init(argc: i32, argv: *const *const c_char, flags: i32);
    unsafe fn doom_update();
}

beskar_lib::entry_point!(main);

fn main() {
    let ex_name = c"doom";
    let argv = [ex_name.as_ptr()];

    doom::game::init();
    doom::screen::init();

    unsafe { doom_init(1, argv.as_ptr(), 0b111) };

    loop {
        unsafe { doom_update() };
        doom::screen::draw();
        doom::input::poll_inputs();
    }
}
