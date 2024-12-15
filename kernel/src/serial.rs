//! Serial port driver for printing debug messages.
//!
//! In QEMU, the serial port can be mapped to the host machine's stdio.
//!
//! On a physical machine, the serial port can be connected to another machine
//! to capture early debug messages in case of hard failure.

#![allow(dead_code)]
#![allow(unused_imports)]

use crate::utils::locks::{MUMcsLock, McsNode};
use core::fmt::Write;

/// Port number for COM1
const COM1_IO_PORT: u16 = 0x3F8;

#[cfg(debug_assertions)]
static SERIAL_PORT: MUMcsLock<SerialPort> = MUMcsLock::uninit();

#[inline]
pub fn init() {
    #[cfg(debug_assertions)]
    SERIAL_PORT.init({
        let s_port = SerialPort::new(COM1_IO_PORT);
        s_port.init();
        s_port
    });
}

#[inline]
/// Write a byte to a serial port.
fn outb(port: u16, data: u8) {
    unsafe {
        core::arch::asm! (
            "out dx, al",
            in("al") data,
            in("dx") port,
            options(nomem, nostack, preserves_flags)
        );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// I/O port-mapped UART
struct SerialPort(u16);

impl SerialPort {
    #[must_use]
    #[inline]
    pub const fn new(base: u16) -> Self {
        Self(base)
    }

    #[must_use]
    #[inline]
    /// Base port number
    const fn port_base(self) -> u16 {
        self.0
    }

    #[must_use]
    #[inline]
    /// Port offsets for specific UART functionality
    const fn port_offset(self, offset: u16) -> u16 {
        self.port_base() + offset
    }

    #[must_use]
    #[inline]
    /// Data port
    const fn port_data(self) -> u16 {
        self.port_offset(0)
    }

    #[must_use]
    #[inline]
    /// Interrupt enable port
    const fn port_int_en(self) -> u16 {
        self.port_offset(1)
    }

    #[must_use]
    #[inline]
    /// Fifo control port
    const fn port_fifo_ctrl(self) -> u16 {
        self.port_offset(2)
    }

    #[must_use]
    #[inline]
    /// Line control port
    const fn port_line_ctrl(self) -> u16 {
        self.port_offset(3)
    }

    #[must_use]
    #[inline]
    /// Modem control port
    const fn port_modem_ctrl(self) -> u16 {
        self.port_offset(4)
    }

    /// Initializes the serial port with default settings.
    pub fn init(self) {
        // Disable interrupts
        outb(self.port_int_en(), 0x00);

        // Enable DLAB to set baud rate
        outb(self.port_line_ctrl(), 0x80);

        // Set baud rate to 38400 (divisor = 3)
        outb(self.port_data(), 0x03); // DLL (low byte of divisor)
        outb(self.port_int_en(), 0x00); // DLM (high byte of divisor)

        // Disable DLAB and configure word length to 8 bits
        outb(self.port_line_ctrl(), 0x03);

        // Enable FIFO, clear TX/RX queues, and set interrupt watermark
        outb(self.port_fifo_ctrl(), 0xC7);

        // Configure modem control: DTR, RTS, and OUT2 (for interrupts)
        outb(self.port_modem_ctrl(), 0x0B);

        // Enable interrupts
        outb(self.port_int_en(), 0x01);
    }

    /// Sends a single byte of data through the serial port.
    pub fn send(self, data: u8) {
        match data {
            8 | 0x7F => {
                // Handle backspace/delete
                self.write_byte(8);
                self.write_byte(b' ');
                self.write_byte(8);
            }
            _ => self.write_byte(data),
        }
    }

    #[inline]
    fn write_byte(self, byte: u8) {
        outb(self.port_data(), byte);
    }
}

impl core::fmt::Write for SerialPort {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for byte in s.bytes() {
            self.send(byte);
        }
        Ok(())
    }
}

#[cfg_attr(not(debug_assertions), allow(unused_variables))]
pub fn serial_print(args: core::fmt::Arguments) {
    #[cfg(debug_assertions)]
    SERIAL_PORT.try_with_locked(|serial| {
        serial.write_fmt(args).unwrap();
    });
}

#[macro_export]
macro_rules! serprint {
    ($($arg:tt)*) => {
        #[cfg(debug_assertions)]
        $crate::serial::serial_print(format_args!($($arg)*));
    };
}

#[macro_export]
macro_rules! sererror {
    ($fmt:expr) => ($crate::serprint!(concat!("[ERROR] ", $fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => ($crate::serprint!(
        concat!("[ERROR] ", $fmt, "\n"), $($arg)*));
}

#[macro_export]
macro_rules! serwarn {
    ($fmt:expr) => ($crate::serprint!(concat!("[WARN ] ", $fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => ($crate::serprint!(
        concat!("[WARN ] ", $fmt, "\n"), $($arg)*));
}

#[macro_export]
macro_rules! serinfo {
    ($fmt:expr) => ($crate::serprint!(concat!("[INFO ] ", $fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => ($crate::serprint!(
        concat!("[INFO ] ", $fmt, "\n"), $($arg)*));
}

#[macro_export]
macro_rules! serdebug {
    ($fmt:expr) => ($crate::serprint!(concat!("[DEBUG] ", $fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => ($crate::serprint!(
        concat!("[DEBUG] ", $fmt, "\n"), $($arg)*));
}

#[macro_export]
macro_rules! sertrace {
    ($fmt:expr) => ($crate::serprint!(concat!("[TRACE] ", $fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => ($crate::serprint!(
        concat!("[TRACE] ", $fmt, "\n"), $($arg)*));
}
