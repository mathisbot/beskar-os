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
pub enum Ps2Error {
    #[error("PS/2 controller self-test failed")]
    SelfTest,
    #[error("PS/2 controller first port test failed")]
    FirstPortTest,
    // #[error("PS/2 controller second port test failed")]
    // SecondPortTest,
    #[error("PS/2 keyboard reset failed")]
    KeyboardReset,
    #[error("PS/2 controller does not support keyboard")]
    KeyboardUnsupported,

    #[error("PS/2 controller data send failed")]
    Sending,
    #[error("PS/2 controller data receive failed")]
    Receiving,
}

impl From<Ps2Error> for beskar_core::drivers::DriverError {
    fn from(error: Ps2Error) -> Self {
        match error {
            Ps2Error::KeyboardUnsupported => Self::Absent,
            Ps2Error::FirstPortTest
            // | Ps2Error::SecondPortTest
            | Ps2Error::KeyboardReset
            | Ps2Error::SelfTest => Self::Invalid,
            Ps2Error::Sending | Ps2Error::Receiving => Self::Unknown,
        }
    }
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
    // EnableSecondPort = 0xA8,
    // TestSecondPort = 0xA9,
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

    pub fn initialize(&self) -> Ps2Result<()> {
        let keyboard_support = ACPI.get().unwrap().fadt().ps2_keyboard();
        PS2_AVAILABLE.store(keyboard_support, Ordering::Relaxed);
        if !keyboard_support {
            video::warn!("PS/2 controller not supported by ACPI");
            return Err(Ps2Error::KeyboardUnsupported);
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
                return Err(Ps2Error::SelfTest);
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
                return Err(Ps2Error::FirstPortTest);
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
        Err(Ps2Error::Sending)
    }

    /// Receive a Device to Host command from the PS/2 controller.
    fn recv(&self) -> Ps2Result<u8> {
        for _ in 0..PS2_RETRIES {
            if self.status_register() & 1 != 0 {
                return Ok(self.read_data());
            }
        }
        Err(Ps2Error::Receiving)
    }

    #[inline]
    /// Send a command to the PS/2 controller and receive a response.
    fn send_recv(&self, value: u8) -> Ps2Result<u8> {
        self.send(value)?;
        self.recv()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[expect(dead_code, reason = "Not all scancode sets are implemented yet")]
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
                return Err(Ps2Error::KeyboardReset);
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
    #[inline]
    pub fn scancode_to_keycode(&self, extended: bool, scancode: u8) -> Option<KeyEvent> {
        match self.scancode_set {
            ScancodeSet::Set1 => Self::scancode_set1_to_keycode(extended, scancode),
            // 2 => self.scancode_set2_to_char(scancode),
            // 3 => self.scancode_set3_to_char(scancode),
            _ => None,
        }
    }

    #[must_use]
    fn scancode_set1_to_keycode(extended: bool, mut scancode: u8) -> Option<KeyEvent> {
        let pressed = if scancode & 0x80 == 0 {
            KeyState::Pressed
        } else {
            scancode &= 0x7F;
            KeyState::Released
        };

        let keycode = match (extended, scancode) {
            (false, 0x01) => Some(KeyCode::Escape),

            (false, 0x1E) => Some(KeyCode::A),
            (false, 0x30) => Some(KeyCode::B),
            (false, 0x2E) => Some(KeyCode::C),
            (false, 0x20) => Some(KeyCode::D),
            (false, 0x12) => Some(KeyCode::E),
            (false, 0x21) => Some(KeyCode::F),
            (false, 0x22) => Some(KeyCode::G),
            (false, 0x23) => Some(KeyCode::H),
            (false, 0x17) => Some(KeyCode::I),
            (false, 0x24) => Some(KeyCode::J),
            (false, 0x25) => Some(KeyCode::K),
            (false, 0x26) => Some(KeyCode::L),
            (false, 0x32) => Some(KeyCode::M),
            (false, 0x31) => Some(KeyCode::N),
            (false, 0x18) => Some(KeyCode::O),
            (false, 0x19) => Some(KeyCode::P),
            (false, 0x10) => Some(KeyCode::Q),
            (false, 0x13) => Some(KeyCode::R),
            (false, 0x1F) => Some(KeyCode::S),
            (false, 0x14) => Some(KeyCode::T),
            (false, 0x16) => Some(KeyCode::U),
            (false, 0x2F) => Some(KeyCode::V),
            (false, 0x11) => Some(KeyCode::W),
            (false, 0x2D) => Some(KeyCode::X),
            (false, 0x15) => Some(KeyCode::Y),
            (false, 0x2C) => Some(KeyCode::Z),

            (false, 0x0B) => Some(KeyCode::Num0),
            (false, 0x02) => Some(KeyCode::Num1),
            (false, 0x03) => Some(KeyCode::Num2),
            (false, 0x04) => Some(KeyCode::Num3),
            (false, 0x05) => Some(KeyCode::Num4),
            (false, 0x06) => Some(KeyCode::Num5),
            (false, 0x07) => Some(KeyCode::Num6),
            (false, 0x08) => Some(KeyCode::Num7),
            (false, 0x09) => Some(KeyCode::Num8),
            (false, 0x0A) => Some(KeyCode::Num9),

            (false, 0x0C) => Some(KeyCode::Minus),
            (false, 0x0D) => Some(KeyCode::Equal),
            (false, 0x1A) => Some(KeyCode::LeftBracket),
            (false, 0x1B) => Some(KeyCode::RightBracket),
            (false, 0x2B) => Some(KeyCode::Backslash),
            (false, 0x27) => Some(KeyCode::Semicolon),
            (false, 0x28) => Some(KeyCode::Apostrophe),
            (false, 0x29) => Some(KeyCode::Tilde),
            (false, 0x33) => Some(KeyCode::Comma),
            (false, 0x34) => Some(KeyCode::Dot),
            (false, 0x35) => Some(KeyCode::Slash),

            (false, 0x1C) => Some(KeyCode::Enter),
            (false, 0x39) => Some(KeyCode::Space),
            (false, 0x0E) => Some(KeyCode::Backspace),
            (false, 0x0F) => Some(KeyCode::Tab),
            (false, 0x3A) => Some(KeyCode::CapsLock),
            (false, 0x2A) => Some(KeyCode::ShiftLeft),
            (false, 0x36) => Some(KeyCode::ShiftRight),
            (false, 0x1D) => Some(KeyCode::CtrlLeft),
            (false, 0x38) => Some(KeyCode::AltLeft),

            (false, 0x3B) => Some(KeyCode::F1),
            (false, 0x3C) => Some(KeyCode::F2),
            (false, 0x3D) => Some(KeyCode::F3),
            (false, 0x3E) => Some(KeyCode::F4),
            (false, 0x3F) => Some(KeyCode::F5),
            (false, 0x40) => Some(KeyCode::F6),
            (false, 0x41) => Some(KeyCode::F7),
            (false, 0x42) => Some(KeyCode::F8),
            (false, 0x43) => Some(KeyCode::F9),
            (false, 0x44) => Some(KeyCode::F10),
            (false, 0x57) => Some(KeyCode::F11),
            (false, 0x58) => Some(KeyCode::F12),

            (false, 0x52) => Some(KeyCode::Numpad0),
            (false, 0x4F) => Some(KeyCode::Numpad1),
            (false, 0x50) => Some(KeyCode::Numpad2),
            (false, 0x51) => Some(KeyCode::Numpad3),
            (false, 0x4B) => Some(KeyCode::Numpad4),
            (false, 0x4C) => Some(KeyCode::Numpad5),
            (false, 0x4D) => Some(KeyCode::Numpad6),
            (false, 0x47) => Some(KeyCode::Numpad7),
            (false, 0x48) => Some(KeyCode::Numpad8),
            (false, 0x49) => Some(KeyCode::Numpad9),
            (false, 0x4E) => Some(KeyCode::NumpadAdd),
            (false, 0x4A) => Some(KeyCode::NumpadSub),
            (false, 0x37) => Some(KeyCode::NumpadMul),
            // (false, 0x35) => Some(KeyCode::NumpadDiv),
            (false, 0x53) => Some(KeyCode::NumpadDot),

            (true, 0x1D) => Some(KeyCode::CtrlRight),

            (true, 0x48) => Some(KeyCode::ArrowUp),
            (true, 0x50) => Some(KeyCode::ArrowDown),
            (true, 0x4B) => Some(KeyCode::ArrowLeft),
            (true, 0x4D) => Some(KeyCode::ArrowRight),

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

    let Some(key_event) = keyboard.scancode_to_keycode(extended, key) else {
        video::warn!("Unknown key: {:#X} (extended: {})", key, extended);
        return;
    };

    super::keyboard::with_keyboard_manager(|manager| {
        manager.push_event(key_event);
    });
}
