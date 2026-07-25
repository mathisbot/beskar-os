#![no_std]

extern crate alloc;

use hyperdrive::once::call_once;

pub mod buffer;
pub mod input;
pub mod render;
pub mod shell;
pub mod theme;

#[cold]
pub fn init() {
    call_once!(shell::init());
}
