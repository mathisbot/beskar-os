#![no_std]
#![no_main]
use beskar_lib::io::keyboard;

beskar_lib::entry_point!(main);

fn main() {
    sh::video::init();

    loop {
        if let Some(event) = keyboard::poll_keyboard() {
            // if event.key() == keyboard::KeyCode::Enter {
            //     beskar_lib::io::print(text.as_str());
            //     text.clear();
            // } else if event.key() == keyboard::KeyCode::Backspace {
            //     text.pop();
            // } else {
            //     let as_char = event.key().as_char();
            //     beskar_lib::println!("read key {}", as_char);
            //     if as_char != '\0' {
            //         text.push(as_char);
            //     }
            // }
            sh::video::tty::with_tty(|tty| tty.handle_key_event(&event));
        } else {
            core::hint::spin_loop();
        }
    }
}
