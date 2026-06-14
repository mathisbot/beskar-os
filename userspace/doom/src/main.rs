#![no_std]
#![no_main]
use core::ffi::c_char;

#[link(name = "puredoom", kind = "static")]
unsafe extern "C" {
    unsafe fn doom_init(argc: i32, argv: *const *const c_char, flags: i32);
    unsafe fn doom_update();
}

beskar_lib::entry_point!(main);

fn main(_start: &beskar_lib::ThreadStartBlock) {
    let ex_name = c"doom";
    let argv = [ex_name.as_ptr()];

    doom::game::init();
    let mut ctx = doom::screen::init();

    unsafe { doom_init(1, argv.as_ptr(), 0b111) };

    let mut keyboard = beskar_lib::io::keyboard::KeyboardReader::new().unwrap();
    loop {
        doom::input::poll_inputs(&mut keyboard);
        unsafe { doom_update() };
        doom::screen::draw(&mut ctx);
    }
}
