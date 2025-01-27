//! Serial port driver for printing debug messages.
//!
//! In QEMU, the serial port can be mapped to the host machine's stdio.
//!
//! On a physical machine, the serial port can be connected to another machine
//! to capture early debug messages in case of hard failure.

use core::marker::PhantomData;

use super::{Access, Port, ReadAccess, ReadWrite, WriteAccess, WriteOnly};

pub mod com;

#[derive(Debug, Clone, PartialEq, Eq)]
/// I/O port-mapped UART
pub struct SerialPort<A: Access> {
    data: Port<u8, ReadWrite>,
    interrupt_enable: Port<u8, WriteOnly>,
    fifo_control: Port<u8, WriteOnly>,
    line_control: Port<u8, WriteOnly>,
    modem_control: Port<u8, WriteOnly>,
    phantom: PhantomData<A>,
}

impl<A: Access> SerialPort<A> {
    #[must_use]
    #[inline]
    pub const fn new(base: u16) -> Self {
        Self {
            data: Port::new(base),
            interrupt_enable: Port::new(base + 1),
            fifo_control: Port::new(base + 2),
            line_control: Port::new(base + 3),
            modem_control: Port::new(base + 4),
            phantom: PhantomData,
        }
    }

    pub fn init(&mut self) {
        // Disable interrupts
        unsafe { self.interrupt_enable.write(0x00) };

        // Enable DLAB to set baud rate
        unsafe { self.line_control.write(0x80) };

        // Set baud rate to 38400 (divisor = 3)
        unsafe {
            self.data.write(0x03); // DLL (low byte of divisor)
            self.interrupt_enable.write(0x00); // DLM (high byte of divisor)
        }

        // Disable DLAB and configure word length to 8 bits
        unsafe { self.line_control.write(0x03) };

        // Enable FIFO, clear TX/RX queues, and set interrupt watermark
        unsafe { self.fifo_control.write(0xC7) };

        // Configure modem control: DTR, RTS, and OUT2 (for interrupts)
        unsafe { self.modem_control.write(0x0B) };

        // Enable interrupts
        unsafe { self.interrupt_enable.write(0x01) };
    }
}

impl<A: ReadAccess> SerialPort<A> {
    /// Receive a single byte of data from the serial port.
    pub fn recv(&mut self) -> u8 {
        unsafe { self.data.read() }
    }
}

impl<A: WriteAccess> SerialPort<A> {
    /// Sends a single byte of data through the serial port.
    pub fn send(&mut self, data: u8) {
        match data {
            8 | 0x7F => {
                // Handle backspace/delete
                unsafe {
                    self.data.write(8);
                    self.data.write(b' ');
                    self.data.write(8);
                }
            }
            _ => unsafe { self.data.write(data) },
        }
    }
}
