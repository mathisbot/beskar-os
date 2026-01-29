use crate::drivers::acpi::ACPI;
use beskar_hal::port::{Port, ReadWrite};
use core::sync::atomic::{AtomicBool, Ordering};
use hyperdrive::locks::ticket::TicketLock;

use super::error::{Ps2Error, Ps2Result};

/// Commands for PS/2 controller communication
#[derive(Debug, Clone, Copy)]
enum Ps2Command {
    DisableFirstPort = 0xAD,
    DisableSecondPort = 0xA7,
    EnableFirstPort = 0xAE,
    TestFirstPort = 0xAB,
    ReadConfigByte = 0x20,
    WriteConfigByte = 0x60,
    SelfTest = 0xAA,

    KeyboardScancodeSet = 0xF0,
    KeyboardEnableScanning = 0xF4,
    KeyboardResend = 0xFE,
    KeyboardResetAndSelfTest = 0xFF,
}

/// Special response bytes from PS/2 devices
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpecialBytes;

impl SpecialBytes {
    pub const ERROR: u8 = 0x00;
    pub const SELF_TEST_PASSED: u8 = 0xAA;
    pub const ECHO: u8 = 0xEE;
    pub const ACK: u8 = 0xFA;
    pub const SELF_TEST_FAIL: u8 = 0xFC;
    pub const SELF_TEST_FAIL2: u8 = 0xFD;
    pub const RESEND: u8 = 0xFE;
    pub const KEY_ERROR: u8 = 0xFF;
}

const PS2_RETRIES: u32 = 1 << 17;

/// PS/2 controller managing port I/O and low-level communication.
pub struct Ps2Controller {
    data_port: TicketLock<Port<u8, ReadWrite>>,
    cmd_sts_port: TicketLock<Port<u8, ReadWrite>>,
    has_two_ports: AtomicBool,
}

impl Ps2Controller {
    const DATA_PORT: u16 = 0x60;
    const CMD_STS_PORT: u16 = 0x64;

    /// Create a new PS/2 controller instance.
    #[must_use]
    #[inline]
    pub const fn new() -> Self {
        Self {
            data_port: TicketLock::new(Port::new(Self::DATA_PORT)),
            cmd_sts_port: TicketLock::new(Port::new(Self::CMD_STS_PORT)),
            has_two_ports: AtomicBool::new(false),
        }
    }

    /// Initialize the PS/2 controller and validate keyboard support.
    pub fn initialize(&self) -> Ps2Result<()> {
        let keyboard_support = ACPI.get().unwrap().fadt().ps2_keyboard();
        if !keyboard_support {
            video::warn!("PS/2 controller not supported by ACPI");
            return Err(Ps2Error::KeyboardUnsupported);
        }

        self.write_command(Ps2Command::DisableFirstPort);
        self.write_command(Ps2Command::DisableSecondPort);
        self.flush_buffer();

        // Read and configure the controller:
        // Bit[1:0]: Interrupt enables (1st port, 2nd port)
        // Bit[6]: Scancode translation (1=Set1, 0=raw set)
        // Bit[4]: Dual-port indicator (1=one port, 0=two ports)
        let mut config = self.read_config();
        config &= !0b11; // Disable interrupts for both ports
        config &= !0b100_0000; // Disable scancode translation (keep raw scancode set)
        let has_two_ports = config & 0b10_000 == 0;
        self.write_config(config);

        // Controller self-test: must respond with 0x55 to pass
        self.write_command(Ps2Command::SelfTest);
        let mut has_passed = false;
        for _ in 0..PS2_RETRIES {
            if self.read_data() == 0x55 {
                has_passed = true;
                break;
            }
        }
        if !has_passed {
            video::warn!("PS/2 controller self-test failed");
            return Err(Ps2Error::SelfTest);
        }
        self.write_config(config);

        // First port test: must respond with 0x00 to pass
        self.write_command(Ps2Command::TestFirstPort);
        has_passed = false;
        for _ in 0..PS2_RETRIES {
            if self.read_data() == 0 {
                has_passed = true;
                break;
            }
        }
        if !has_passed {
            video::warn!("PS/2 controller first port test failed");
            return Err(Ps2Error::FirstPortTest);
        }

        // Enable the first port: set interrupt and clock bits for port 1
        self.write_command(Ps2Command::EnableFirstPort);
        self.write_config((config | 0b1) & !0b1_0000);

        self.has_two_ports.store(has_two_ports, Ordering::Relaxed);

        Ok(())
    }

    /// Read a byte from the data port.
    #[must_use]
    #[inline]
    pub fn read_data(&self) -> u8 {
        unsafe { self.data_port.lock().read() }
    }

    /// Write a byte to the data port.
    #[inline]
    pub fn write_data(&self, data: u8) {
        unsafe { self.data_port.lock().write(data) };
    }

    /// Read the status register.
    #[must_use]
    #[inline]
    pub fn read_status(&self) -> u8 {
        unsafe { self.cmd_sts_port.lock().read() }
    }

    /// Send data to a PS/2 device, waiting for the controller's input buffer to be ready.
    ///
    /// Polls the status register's "input buffer full" bit (0x02). When clear,
    /// the input buffer is empty and ready to accept the next command/data byte.
    pub fn send(&self, value: u8) -> Ps2Result<()> {
        for _ in 0..PS2_RETRIES {
            if self.read_status() & 0b10 == 0 {
                self.write_data(value);
                return Ok(());
            }
        }
        Err(Ps2Error::Sending)
    }

    /// Receive data from a PS/2 device, waiting for the controller's output buffer to contain data.
    ///
    /// Polls the status register's "output buffer full" bit (0x01). When set,
    /// the output buffer contains data ready to be read.
    pub fn recv(&self) -> Ps2Result<u8> {
        for _ in 0..PS2_RETRIES {
            if self.read_status() & 1 != 0 {
                return Ok(self.read_data());
            }
        }
        Err(Ps2Error::Receiving)
    }

    /// Send a command and receive a response.
    #[inline]
    pub fn send_recv(&self, value: u8) -> Ps2Result<u8> {
        self.send(value)?;
        self.recv()
    }

    #[inline]
    fn write_command(&self, command: Ps2Command) {
        unsafe { self.cmd_sts_port.lock().write(command as u8) };
    }

    #[must_use]
    #[inline]
    fn read_config(&self) -> u8 {
        self.write_command(Ps2Command::ReadConfigByte);
        self.read_data()
    }

    #[inline]
    fn write_config(&self, config: u8) {
        self.write_command(Ps2Command::WriteConfigByte);
        self.write_data(config);
    }

    #[inline]
    fn flush_buffer(&self) {
        let _ = self.read_data();
    }
}

impl Default for Ps2Controller {
    fn default() -> Self {
        Self::new()
    }
}
