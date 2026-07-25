#![no_std]
#![no_main]

use beskar_lib::{
    io::keyboard,
    time::{Duration, Instant},
};

beskar_lib::entry_point!(main);

fn main(_start: &beskar_lib::ThreadStartBlock) {
    const IDLE_THRESHOLD: Duration = Duration::from_millis(200);

    bashkar::init();

    let mut shell = bashkar::shell::Shell::new();
    let mut last_input = Instant::now();

    loop {
        if let Some(event) = keyboard::poll_keyboard() {
            shell.handle_key(&event);
            last_input = Instant::now();
        } else if last_input.elapsed() >= IDLE_THRESHOLD {
            keyboard::wait_next_event();
        } else {
            core::hint::spin_loop();
        }
    }
}
