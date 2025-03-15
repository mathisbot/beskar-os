#![no_std]
#![no_main]

use beskar_lib::io::print;

beskar_lib::entry_point!(main);

fn main() {
    print("Hello, userspace!");

    let mut buffer = [42_u8; 16];
    beskar_lib::rand::rand(&mut buffer);
}
