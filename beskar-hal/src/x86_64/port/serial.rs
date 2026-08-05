//! Serial port driver for printing debug messages.
//!
//! In QEMU, the serial port can be mapped to the host machine's stdio.
//!
//! On a physical machine, the serial port can be connected to another machine
//! to capture early debug messages in case of hard failure.
use super::{Access, Port, ReadAccess, ReadOnly, ReadWrite, WriteAccess, WriteOnly};
use core::marker::PhantomData;
use thiserror::Error;

pub mod com;

#[derive(Debug, Clone, PartialEq, Eq)]
/// I/O port-mapped UART
pub struct SerialPort<A: Access> {
    data: Port<u8, ReadWrite>,
    interrupt_enable: Port<u8, WriteOnly>,
    fifo_control: Port<u8, WriteOnly>,
    line_control: Port<u8, WriteOnly>,
    modem_control: Port<u8, WriteOnly>,
    line_status: Port<u8, ReadOnly>,
    phantom: PhantomData<A>,
}

impl<A: Access> SerialPort<A> {
    /// Indicates that data is available to read from the serial port
    const LINE_STATUS_DATA_READY: u8 = 1 << 0;
    // /// Indicates that an overrun error has occurred (data was lost)
    // const LINE_STATUS_OVERRUN: u8 = 1 << 1;
    // /// Indicates that a parity error has occurred
    // const LINE_STATUS_PARITY_ERROR: u8 = 1 << 2;
    // /// Indicates that a framing error has occurred (invalid stop bit)
    // const LINE_STATUS_FRAMING_ERROR: u8 = 1 << 3;
    // /// Indicates that a break interrupt has occurred (line held low for too long)
    // const LINE_STATUS_BREAK_INTERRUPT: u8 = 1 << 4;
    /// Indicates that the transmitter holding register is empty and ready for new data
    const LINE_STATUS_THR_EMPTY: u8 = 1 << 5;
    // /// Indicates that the transmitter is empty
    // const LINE_STATUS_TRANSMITTER_EMPTY: u8 = 1 << 6;

    const SPIN_LIMIT: usize = 10_000;

    #[must_use]
    #[inline]
    pub const fn new(base: u16) -> Self {
        Self {
            data: Port::new(base),
            interrupt_enable: Port::new(base + 1),
            fifo_control: Port::new(base + 2),
            line_control: Port::new(base + 3),
            modem_control: Port::new(base + 4),
            line_status: Port::new(base + 5),
            phantom: PhantomData,
        }
    }

    #[must_use]
    #[inline]
    fn line_status(&self) -> u8 {
        unsafe { self.line_status.read() }
    }

    #[must_use]
    #[inline]
    fn test_line_status(&self, mask: u8) -> bool {
        self.line_status() & mask != 0
    }

    fn wait_for_line_status(&self, mask: u8) {
        while !self.test_line_status(mask) {
            core::hint::spin_loop();
        }
    }

    fn try_wait_for_line_status(&self, mask: u8) -> bool {
        for _ in 0..Self::SPIN_LIMIT {
            if self.test_line_status(mask) {
                return true;
            }
            core::hint::spin_loop();
        }
        false
    }

    pub fn init(&mut self) -> SerialResult<()> {
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

        // Perform a self-test
        unsafe {
            self.modem_control.write(0x1E); // Set loopback mode
            self.data.write(0xAE); // Send test pattern
            if self.data.read() != 0xAE {
                // Check test pattern
                return Err(SerialError::Unavailable);
            }
        }

        // Enable IRQ and OUT1/2
        unsafe { self.modem_control.write(0x0F) };

        Ok(())
    }
}

impl<A: ReadAccess> SerialPort<A> {
    /// Receive a single byte of data from the serial port.
    pub fn recv(&mut self) -> u8 {
        self.wait_for_line_status(Self::LINE_STATUS_DATA_READY);
        unsafe { self.data.read() }
    }

    pub fn try_recv(&mut self) -> Option<u8> {
        self.try_wait_for_line_status(Self::LINE_STATUS_DATA_READY)
            .then(|| unsafe { self.data.read() })
    }
}

impl<A: WriteAccess> SerialPort<A> {
    /// Sends a single byte of data through the serial port.
    pub fn send(&mut self, data: u8) {
        match data {
            8 | 0x7F => {
                // Handle backspace/delete
                self.send_byte(8);
                self.send_byte(b' ');
                self.send_byte(8);
            }
            _ => self.send_byte(data),
        }
    }

    /// Tries  to send a single byte of data through the serial port.
    pub fn try_send(&mut self, data: u8) -> bool {
        match data {
            8 | 0x7F => {
                // Handle backspace/delete
                self.try_send_byte(8) && self.try_send_byte(b' ') && self.try_send_byte(8)
            }
            _ => self.try_send_byte(data),
        }
    }

    fn send_byte(&mut self, data: u8) {
        self.wait_for_line_status(Self::LINE_STATUS_THR_EMPTY);
        unsafe { self.data.write(data) };
    }

    fn try_send_byte(&mut self, data: u8) -> bool {
        if !self.try_wait_for_line_status(Self::LINE_STATUS_THR_EMPTY) {
            return false;
        }
        unsafe { self.data.write(data) };
        true
    }
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
/// Error type for serial port operations
pub enum SerialError {
    #[error("Serial port is not available")]
    Unavailable,
}

pub type SerialResult<T> = Result<T, SerialError>;
