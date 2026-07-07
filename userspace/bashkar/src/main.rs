#![no_std]
#![no_main]

use beskar_core::time::Duration;
use beskar_lib::{
    io::keyboard,
    time::{elapsed, now},
};

beskar_lib::entry_point!(main);

fn main(_start: &beskar_lib::ThreadStartBlock) {
    const IDLE_THRESHOLD: Duration = Duration::from_millis(200);

    let mut shell = bashkar::shell::Shell::new();
    let mut last_input = now();

    loop {
        if let Some(event) = keyboard::poll_keyboard() {
            shell.handle_key(&event);
            last_input = now();
        } else if elapsed(last_input) >= IDLE_THRESHOLD {
            keyboard::wait_next_event();
        } else {
            core::hint::spin_loop();
        }
    }
}
