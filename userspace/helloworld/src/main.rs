#![no_std]
#![no_main]

use beskar_lib::{io::print, rand::rand};

beskar_lib::entry_point!(main);

fn main() {
    print("Hello, userspace!");

    // Safety: any 8 random bytes are valid u64 values.
    let _random_u64 = unsafe { rand::<u64>() };
}
