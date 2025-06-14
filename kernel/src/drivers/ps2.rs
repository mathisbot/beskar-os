use crate::drivers::acpi::ACPI;
use beskar_core::drivers::{
    DriverResult,
    keyboard::{KeyCode, KeyEvent, KeyState},
};
use beskar_hal::port::{Port, ReadWrite};
use core::sync::atomic::{AtomicBool, Ordering};
use hyperdrive::{locks::ticket::TicketLock, once::Once};
use thiserror::Error;

static PS2_AVAILABLE: AtomicBool = AtomicBool::new(false);

static PS2_CONTROLLER: Ps2Controller = Ps2Controller::new();
static PS2_KEYBOARD: Once<Ps2Keyboard> = Once::uninit();

const PS2_RETRIES: u32 = 1 << 17;

pub fn init() -> DriverResult<()> {
    PS2_CONTROLLER.initialize()?;
    let ps2_keyboard = Ps2Keyboard::new(&PS2_CONTROLLER)?;
    PS2_KEYBOARD.call_once(|| ps2_keyboard);
    video::info!("PS/2 controller initialized");
    Ok(())
}

#[derive(Error, Debug, Clone, Copy)]
enum Ps2Error {
    #[error("PS/2 controller self-test failed")]
    SelfTestFailed,
    #[error("PS/2 controller first port test failed")]
    FirstPortTestFailed,
    #[error("PS/2 controller second port test failed")]
    SecondPortTestFailed,
    #[error("PS/2 keyboard reset failed")]
    KeyboardResetFailed,

    #[error("PS/2 controller data send failed")]
    SendingFailed,
    #[error("PS/2 controller data receive failed")]
    ReceivingFailed,
}

type Ps2Result<T> = Result<T, Ps2Error>;

pub struct Ps2Controller {
    data_port: TicketLock<Port<u8, ReadWrite>>,
    cmd_sts_port: TicketLock<Port<u8, ReadWrite>>,
    has_two_ports: AtomicBool,
}

enum Ps2Command {
    DisableFirstPort = 0xAD,
    DisableSecondPort = 0xA7,
    EnableFirstPort = 0xAE,
    EnableSecondPort = 0xA8,
    TestSecondPort = 0xA9,
    TestFirstPort = 0xAB,
    ReadConfigByte = 0x20,
    WriteConfigByte = 0x60,
    SelfTest = 0xAA,

    KeyboardScancodeSet = 0xF0,
    KeyboardResetAndSelfTest = 0xFF,
}

impl Default for Ps2Controller {
    fn default() -> Self {
        Self::new()
    }
}

impl Ps2Controller {
    const DATA_PORT: u16 = 0x60;
    const CMD_STS_PORT: u16 = 0x64;

    #[must_use]
    #[inline]
    pub const fn new() -> Self {
        Self {
            data_port: TicketLock::new(Port::new(Self::DATA_PORT)),
            cmd_sts_port: TicketLock::new(Port::new(Self::CMD_STS_PORT)),
            has_two_ports: AtomicBool::new(false),
        }
    }

    #[inline]
    fn write_config(&self, config: u8) {
        self.write_command(Ps2Command::WriteConfigByte);
        self.write_data(config);
    }

    #[must_use]
    #[inline]
    fn read_config(&self) -> u8 {
        self.write_command(Ps2Command::ReadConfigByte);
        self.read_data()
    }

    #[must_use]
    #[inline]
    fn status_register(&self) -> u8 {
        unsafe { self.cmd_sts_port.lock().read() }
    }

    #[inline]
    fn flush_buffer(&self) {
        let _ = self.read_data();
    }

    pub fn initialize(&self) -> DriverResult<()> {
        let keyboard_support = ACPI.get().unwrap().fadt().ps2_keyboard();
        PS2_AVAILABLE.store(keyboard_support, Ordering::Relaxed);
        if !keyboard_support {
            video::warn!("PS/2 controller not supported by ACPI");
            return Err(beskar_core::drivers::DriverError::Absent);
        }

        self.write_command(Ps2Command::DisableFirstPort);
        self.write_command(Ps2Command::DisableSecondPort);

        self.flush_buffer();

        // Set up the controller configuration byte
        let mut config = self.read_config();
        config &= !0b11; // Disable interrupts for both ports
        config &= !0b100_0000; // Disable scancode translation
        let has_two_ports = config & 0b10_000 == 0;
        self.write_config(config);

        // Perform self-test
        self.write_command(Ps2Command::SelfTest);
        {
            let mut has_passed = false;
            for _ in 0..PS2_RETRIES {
                if self.read_data() == 0x55 {
                    has_passed = true;
                    break;
                }
            }
            if !has_passed {
                video::warn!("PS/2 controller self-test failed");
                return Err(beskar_core::drivers::DriverError::Invalid);
            }
        }
        self.write_config(config);

        // Perform first port test
        self.write_command(Ps2Command::TestFirstPort);
        {
            let mut has_passed = false;
            for _ in 0..PS2_RETRIES {
                if self.read_data() == 0 {
                    has_passed = true;
                    break;
                }
            }
            if !has_passed {
                video::warn!("PS/2 controller first port test failed");
                return Err(beskar_core::drivers::DriverError::Invalid);
            }
        }

        // Enable the first port
        self.write_command(Ps2Command::EnableFirstPort);
        self.write_config((config | 1) & !0b1_0000); // Enable interrupts/clock for the first port

        self.has_two_ports.store(has_two_ports, Ordering::Relaxed);

        Ok(())
    }

    #[inline]
    fn write_command(&self, command: Ps2Command) {
        unsafe { self.cmd_sts_port.lock().write(command as u8) };
    }

    #[must_use]
    #[inline]
    fn read_status(&self) -> u8 {
        unsafe { self.cmd_sts_port.lock().read() }
    }

    #[inline]
    fn write_data(&self, data: u8) {
        unsafe { self.data_port.lock().write(data) };
    }

    #[must_use]
    #[inline]
    fn read_data(&self) -> u8 {
        unsafe { self.data_port.lock().read() }
    }

    /// Send a Host to Device command to the PS/2 controller.
    fn send(&self, value: u8) -> Ps2Result<()> {
        for _ in 0..PS2_RETRIES {
            if self.status_register() & 0b10 == 0 {
                self.write_data(value);
                return Ok(());
            }
        }
        Err(Ps2Error::SendingFailed)
    }

    /// Receive a Device to Host command from the PS/2 controller.
    fn recv(&self) -> Ps2Result<u8> {
        for _ in 0..PS2_RETRIES {
            if self.status_register() & 1 != 0 {
                return Ok(self.read_data());
            }
        }
        Err(Ps2Error::ReceivingFailed)
    }

    #[inline]
    /// Send a command to the PS/2 controller and receive a response.
    fn send_recv(&self, value: u8) -> Ps2Result<u8> {
        self.send(value)?;
        self.recv()
    }
}

enum ScancodeSet {
    Set1 = 1,
    Set2 = 2,
    Set3 = 3,
}

pub struct Ps2Keyboard<'c> {
    controller: &'c Ps2Controller,
    scancode_set: ScancodeSet,
}

impl<'a> Ps2Keyboard<'a> {
    #[inline]
    pub fn new(controller: &'a Ps2Controller) -> DriverResult<Self> {
        const DEFAULT_SCANCODE_SET: ScancodeSet = ScancodeSet::Set1;

        let keyboard = Ps2Keyboard {
            controller,
            scancode_set: DEFAULT_SCANCODE_SET,
        };

        // Reset the keyboard
        let Ok(value) = keyboard.send_command(Ps2Command::KeyboardResetAndSelfTest as u8) else {
            video::warn!("PS/2 keyboard failed to receive reset command");
            return Err(beskar_core::drivers::DriverError::Invalid);
        };
        if value != 0xFA {
            video::warn!("PS/2 keyboard didn't acknowledge");
            return Err(beskar_core::drivers::DriverError::Invalid);
        }
        {
            let mut has_passed = false;
            for _ in 0..PS2_RETRIES {
                let value = controller.read_data();
                if value == 0xAA {
                    has_passed = true;
                    break;
                }
                if value == 0xFC || value == 0xFD {
                    video::warn!("PS/2 keyboard reset failed");
                    return Err(beskar_core::drivers::DriverError::Invalid);
                }
            }
            if !has_passed {
                video::warn!("PS/2 keyboard reset failed with unexpected value");
                return Err(beskar_core::drivers::DriverError::Unknown);
            }
        }

        // Set the scancode set
        let res = keyboard
            .send_command(Ps2Command::KeyboardScancodeSet as u8)
            .unwrap();
        if res != 0xFA {
            video::warn!("PS/2 keyboard didn't acknowledge scancode set command");
            return Err(beskar_core::drivers::DriverError::Invalid);
        }
        let res = keyboard.send_command(DEFAULT_SCANCODE_SET as u8).unwrap();
        if res != 0xFA {
            video::warn!("PS/2 keyboard didn't acknowledge scancode set command");
            return Err(beskar_core::drivers::DriverError::Invalid);
        }

        Ok(keyboard)
    }

    fn send_command(&self, command: u8) -> Ps2Result<u8> {
        const TRIES: u8 = 4;
        for _ in 0..TRIES {
            let Ok(value) = self.controller.send_recv(command) else {
                continue;
            };
            if value == 0xFA || value == 0xAA {
                // Acknowledge or SelfTestPass
                return Ok(value);
            } else if value == 0xFE {
                // Resend
            } else if value == 0xFC || value == 0xFD {
                // SelfTestFail
                return Err(Ps2Error::KeyboardResetFailed);
            }
        }
        // Try one last time to propagate error
        self.controller.send_recv(command)
    }

    #[must_use]
    fn read_scancode(&self) -> Option<u8> {
        // Check if data is available
        let status = self.controller.read_status();
        if status & 1 != 0 {
            Some(self.controller.read_data())
        } else {
            None
        }
    }

    #[must_use]
    pub fn poll_key(&self) -> Option<u8> {
        self.read_scancode()
    }

    #[must_use]
    pub fn scancode_to_keycode(&self, scancode: u8) -> Option<KeyEvent> {
        match self.scancode_set {
            ScancodeSet::Set1 => Self::scancode_set1_to_keycode(scancode),
            // 2 => self.scancode_set2_to_char(scancode),
            // 3 => self.scancode_set3_to_char(scancode),
            _ => None,
        }
    }

    #[must_use]
    fn scancode_set1_to_keycode(mut scancode: u8) -> Option<KeyEvent> {
        let pressed = if scancode & 0x80 == 0 {
            KeyState::Pressed
        } else {
            scancode &= 0x7F;
            KeyState::Released
        };

        let keycode = match scancode {
            0x1E => Some(KeyCode::A),
            0x30 => Some(KeyCode::B),
            0x2E => Some(KeyCode::C),
            0x20 => Some(KeyCode::D),
            0x12 => Some(KeyCode::E),
            0x21 => Some(KeyCode::F),
            0x22 => Some(KeyCode::G),
            0x23 => Some(KeyCode::H),
            0x17 => Some(KeyCode::I),
            0x24 => Some(KeyCode::J),
            0x25 => Some(KeyCode::K),
            0x26 => Some(KeyCode::L),
            0x32 => Some(KeyCode::M),
            0x31 => Some(KeyCode::N),
            0x18 => Some(KeyCode::O),
            0x19 => Some(KeyCode::P),
            0x10 => Some(KeyCode::Q),
            0x13 => Some(KeyCode::R),
            0x1F => Some(KeyCode::S),
            0x14 => Some(KeyCode::T),
            0x16 => Some(KeyCode::U),
            0x2F => Some(KeyCode::V),
            0x11 => Some(KeyCode::W),
            0x2D => Some(KeyCode::X),
            0x15 => Some(KeyCode::Y),
            0x2C => Some(KeyCode::Z),

            0x0B => Some(KeyCode::Num0),
            0x02 => Some(KeyCode::Num1),
            0x03 => Some(KeyCode::Num2),
            0x04 => Some(KeyCode::Num3),
            0x05 => Some(KeyCode::Num4),
            0x06 => Some(KeyCode::Num5),
            0x07 => Some(KeyCode::Num6),
            0x08 => Some(KeyCode::Num7),
            0x09 => Some(KeyCode::Num8),
            0x0A => Some(KeyCode::Num9),

            0x0C => Some(KeyCode::Minus),
            0x0D => Some(KeyCode::Equal),
            0x1A => Some(KeyCode::LeftBracket),
            0x1B => Some(KeyCode::RightBracket),
            0x2B => Some(KeyCode::Backslash),
            0x27 => Some(KeyCode::Semicolon),
            0x28 => Some(KeyCode::Apostrophe),
            0x29 => Some(KeyCode::Tilde),
            0x33 => Some(KeyCode::Comma),
            0x34 => Some(KeyCode::Dot),
            0x35 => Some(KeyCode::Slash),

            0x1C => Some(KeyCode::Enter),
            0x39 => Some(KeyCode::Space),
            0x0E => Some(KeyCode::Backspace),
            0x0F => Some(KeyCode::Tab),
            0x3A => Some(KeyCode::CapsLock),
            0x2A => Some(KeyCode::ShiftLeft),
            0x36 => Some(KeyCode::ShiftRight),
            0x1D => Some(KeyCode::CtrlLeft),
            0x38 => Some(KeyCode::AltLeft),

            0x3B => Some(KeyCode::F1),
            0x3C => Some(KeyCode::F2),
            0x3D => Some(KeyCode::F3),
            0x3E => Some(KeyCode::F4),
            0x3F => Some(KeyCode::F5),
            0x40 => Some(KeyCode::F6),
            0x41 => Some(KeyCode::F7),
            0x42 => Some(KeyCode::F8),
            0x43 => Some(KeyCode::F9),
            0x44 => Some(KeyCode::F10),
            0x57 => Some(KeyCode::F11),
            0x58 => Some(KeyCode::F12),

            0x52 => Some(KeyCode::Numpad0),
            0x4F => Some(KeyCode::Numpad1),
            0x50 => Some(KeyCode::Numpad2),
            0x51 => Some(KeyCode::Numpad3),
            0x4B => Some(KeyCode::Numpad4),
            0x4C => Some(KeyCode::Numpad5),
            0x4D => Some(KeyCode::Numpad6),
            0x47 => Some(KeyCode::Numpad7),
            0x48 => Some(KeyCode::Numpad8),
            0x49 => Some(KeyCode::Numpad9),
            0x4E => Some(KeyCode::NumpadAdd),
            0x4A => Some(KeyCode::NumpadSub),
            0x37 => Some(KeyCode::NumpadMul),
            // 0x35 => Some(KeyCode::NumpadDiv),
            0x53 => Some(KeyCode::NumpadDot),

            // TODO: Modifier keys
            _ => None,
        }?;

        Some(KeyEvent::new(keycode, pressed))
    }
}

// pub struct Ps2Mouse<'c> {
//     controller: &'c Ps2Controller,
// }

// impl<'a> Ps2Mouse<'a> {
//     #[must_use]
//     #[inline]
//     pub const fn new(controller: &'a Ps2Controller) -> Self {
//         Ps2Mouse { controller }
//     }

//     pub fn initialize(&self) {
//         // Enable mouse device
//         self.controller.write_command(0xD4); // Send command to mouse
//         self.controller.write_data(0xF4); // Enable data reporting
//     }

//     #[must_use]
//     pub fn read_packet(&self) -> Option<[u8; 3]> {
//         // Check if data is available
//         let status = self.controller.read_command();
//         if status & 1 != 0 {
//             let mut packet = [0u8; 3];
//             for byte in &mut packet {
//                 *byte = self.controller.read_data();
//             }
//             Some(packet)
//         } else {
//             None
//         }
//     }
// }

pub fn handle_keyboard_interrupt() {
    static EXTENDED: AtomicBool = AtomicBool::new(false);

    let Some(keyboard) = PS2_KEYBOARD.get() else {
        return;
    };

    let Some(scan_code) = keyboard.poll_key() else {
        return;
    };

    if scan_code == 0xE0 {
        let previous = EXTENDED.swap(true, Ordering::Release);
        assert!(!previous);
        // Do nothing, wait for the next byte
    } else if scan_code == 0xE1 {
        // TODO: Handle pause/break key
        EXTENDED.store(true, Ordering::Release);
    } else {
        let extended = EXTENDED.swap(false, Ordering::Release);
        if scan_code != 0 && scan_code != 0xFA {
            handle_real_key(extended, scan_code);
        }
    }
}

fn handle_real_key(extended: bool, key: u8) {
    let keyboard = PS2_KEYBOARD.get().unwrap();

    let Some(key_event) = keyboard.scancode_to_keycode(key) else {
        video::warn!("Unknown key: {:#X}", key);
        return;
    };

    super::keyboard::with_keyboard_manager(|manager| {
        manager.push_event(key_event);
    });
}
