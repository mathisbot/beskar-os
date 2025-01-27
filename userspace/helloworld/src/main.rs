#![no_std]
#![no_main]

use beskar_lib::io::print;

beskar_lib::entry_point!(main);

fn main() {
    print("Hello, userspace!");
}
