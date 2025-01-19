//! Implementation of the serial communication interface

use super::SerialPort;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u16)]
pub enum ComNumber {
    Com1 = 0x3F8,
    Com2 = 0x2F8,
    Com3 = 0x3E8,
    Com4 = 0x2E8,
}

impl ComNumber {
    #[must_use]
    #[inline]
    pub const fn io_port(self) -> u16 {
        self as u16
    }
}

pub struct SerialCom(SerialPort);

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

impl core::ops::Deref for SerialCom {
    type Target = SerialPort;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl core::ops::DerefMut for SerialCom {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl core::fmt::Write for SerialCom {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for byte in s.bytes() {
            self.send(byte);
        }
        Ok(())
    }
}
