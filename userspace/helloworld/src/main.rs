#![no_std]
#![no_main]

use beskar_lib::{
    io::{keyboard, print},
    rand::rand,
};

beskar_lib::entry_point!(main);

fn main() {
    print("Hello, userspace!");

    // Safety: any 8 random bytes are valid u64 values.
    let random_u64 = unsafe { rand::<u64>() };

    let _test_vec = alloc::vec![0; 10];

    beskar_lib::println!("Random u64: {:#x}", random_u64);

    let mut text = alloc::string::String::new();
    loop {
        if let Some(event) = keyboard::poll_keyboard() {
            if event.pressed() != keyboard::KeyState::Pressed {
                continue;
            }

            if event.key() == keyboard::KeyCode::Enter {
                beskar_lib::io::print(text.as_str());
                text.clear();
            } else if event.key() == keyboard::KeyCode::Backspace {
                text.pop();
            } else {
                let as_char = event.key().as_char();
                beskar_lib::println!("read key {}", as_char);
                if as_char != '\0' {
                    text.push(as_char);
                }
            }
        } else {
            core::hint::spin_loop();
        }
    }
}
