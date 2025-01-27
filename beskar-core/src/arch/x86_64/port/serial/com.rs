//! Implementation of the serial communication interface

use super::super::WriteOnly;
use super::SerialPort;

/// Port number for COM1
const COM1_IO_PORT: u16 = 0x3F8;
/// Port number for COM2
const COM2_IO_PORT: u16 = 0x2F8;
/// Port number for COM3
const COM3_IO_PORT: u16 = 0x3E8;
/// Port number for COM4
const COM4_IO_PORT: u16 = 0x2E8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComNumber {
    Com1,
    Com2,
    Com3,
    Com4,
}

impl ComNumber {
    #[must_use]
    #[inline]
    pub const fn io_port(&self) -> u16 {
        match self {
            Self::Com1 => COM1_IO_PORT,
            Self::Com2 => COM2_IO_PORT,
            Self::Com3 => COM3_IO_PORT,
            Self::Com4 => COM4_IO_PORT,
        }
    }
}

pub struct SerialCom(SerialPort<WriteOnly>);

impl Default for SerialCom {
    fn default() -> Self {
        Self::new(ComNumber::Com1)
    }
}

impl SerialCom {
    #[must_use]
    #[inline]
    pub const fn new(com: ComNumber) -> Self {
        Self(SerialPort::new(com.io_port()))
    }

    pub fn init(&mut self) {
        self.0.init();
    }
}

impl core::fmt::Write for SerialCom {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for byte in s.bytes() {
            self.0.send(byte);
        }
        Ok(())
    }
}
