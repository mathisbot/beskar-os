#![no_std]
#![no_main]

use beskar_lib::{io::print, rand::rand};

beskar_lib::entry_point!(main);

fn main() {
    print("Hello, userspace!");

    // Safety: any 8 random bytes are valid u64 values.
    let random_u64 = unsafe { rand::<u64>() };

    let _test_vec = alloc::vec![0; 10];

    beskar_lib::println!("Random u64: {:#x}", random_u64);

    loop {
        if let Some(event) = beskar_lib::io::poll_keyboard() {
            let key = event.key();
            let pressed = event.pressed();
            beskar_lib::println!("Key: {:?} Pressed: {:?}", key, pressed,);
        } else {
            core::hint::spin_loop();
        }
    }
}
