use super::controller::{Ps2Controller, SpecialBytes};
use super::error::{Ps2Error, Ps2Result};
use beskar_core::drivers::{DriverResult, keyboard::KeyEvent};
use core::sync::atomic::{AtomicU16, Ordering};

mod decoder;
use decoder::{ScancodeDecoder, ScancodeSet};
mod keycodes;

const PS2_RETRIES: u32 = 1 << 17;

/// PS/2 keyboard device driver.
pub struct Ps2Keyboard<'c> {
    controller: &'c Ps2Controller,
    scancode_set: ScancodeSet,
    state: AtomicU16,
}

impl<'a> Ps2Keyboard<'a> {
    /// Create and initialize a new PS/2 keyboard device.
    pub fn new(controller: &'a Ps2Controller) -> DriverResult<Self> {
        const DEFAULT_SCANCODE_SET: ScancodeSet = ScancodeSet::Set2;

        let mut keyboard = Ps2Keyboard {
            controller,
            scancode_set: DEFAULT_SCANCODE_SET,
            state: AtomicU16::new(0),
        };

        keyboard.reset_device()?;
        keyboard.flush_output_buffer();
        keyboard.scancode_set = keyboard.ensure_scancode_set(DEFAULT_SCANCODE_SET);
        keyboard.enable_scanning()?;

        video::debug!("PS/2 keyboard scancode set: {:?}", keyboard.scancode_set);

        Ok(keyboard)
    }

    /// Poll for a key event from the keyboard.
    #[must_use]
    pub fn poll_key_event(&self) -> Option<KeyEvent> {
        let scancode = self.read_scancode()?;
        let mut state = self.state.load(Ordering::Acquire);
        let event = ScancodeDecoder::decode(scancode, &mut state, self.scancode_set);
        self.state.store(state, Ordering::Release);
        event
    }

    /// Reset and self-test the keyboard device.
    fn reset_device(&self) -> DriverResult<()> {
        let Ok(value) = self.send_command(0xFF) else {
            video::warn!("PS/2 keyboard failed to receive reset command");
            return Err(beskar_core::drivers::DriverError::Invalid);
        };

        if value != SpecialBytes::ACK {
            video::warn!("PS/2 keyboard didn't acknowledge");
            return Err(beskar_core::drivers::DriverError::Invalid);
        }

        let mut has_passed = false;
        for _ in 0..PS2_RETRIES {
            let value = self.controller.read_data();
            if value == SpecialBytes::SELF_TEST_PASSED {
                has_passed = true;
                break;
            }
            if value == SpecialBytes::SELF_TEST_FAIL || value == SpecialBytes::SELF_TEST_FAIL2 {
                video::warn!("PS/2 keyboard reset failed");
                return Err(beskar_core::drivers::DriverError::Invalid);
            }
        }

        if !has_passed {
            video::warn!("PS/2 keyboard reset failed with unexpected value");
            return Err(beskar_core::drivers::DriverError::Unknown);
        }

        Ok(())
    }

    /// Enable keyboard scanning.
    fn enable_scanning(&self) -> DriverResult<()> {
        let res = self.send_command(0xF4)?;
        if res != SpecialBytes::ACK {
            video::warn!("PS/2 keyboard didn't acknowledge scanning enable command");
            return Err(beskar_core::drivers::DriverError::Invalid);
        }
        Ok(())
    }

    /// Attempt to use the preferred scancode set, falling back gracefully if unsupported.
    ///
    /// Some older or non-standard keyboards may not support all three sets.
    /// This queries the current set, tries to switch if needed, then validates the result.
    /// Returns the successfully active set or the preferred default if switching fails.
    fn ensure_scancode_set(&self, preferred: ScancodeSet) -> ScancodeSet {
        let detected = self.query_scancode_set().ok();
        if detected == Some(preferred) {
            return preferred;
        }

        if self.set_scancode_set(preferred).is_ok() {
            return self.query_scancode_set().unwrap_or(preferred);
        }

        detected.unwrap_or(preferred)
    }

    /// Query the current scancode set.
    fn query_scancode_set(&self) -> Ps2Result<ScancodeSet> {
        let res = self.send_command(0xF0)?;
        if res != 0xFA {
            return Err(Ps2Error::Receiving);
        }

        let res = self.send_command(0x00)?;
        if res != 0xFA {
            return Err(Ps2Error::Receiving);
        }

        let value = self.controller.recv()?;
        ScancodeSet::try_from(value).map_err(|()| Ps2Error::Receiving)
    }

    /// Switch to a new scancode set.
    fn set_scancode_set(&self, set: ScancodeSet) -> Ps2Result<()> {
        let res = self.send_command(0xF0)?;
        if res != 0xFA {
            return Err(Ps2Error::Sending);
        }

        let res = self.send_command(set as u8)?;
        if res != 0xFA {
            return Err(Ps2Error::Sending);
        }

        Ok(())
    }

    /// Send a command to the keyboard and retrieve the response, with retries on resend requests.
    fn send_command(&self, command: u8) -> Ps2Result<u8> {
        const TRIES: u8 = 4;
        for _ in 0..TRIES {
            let value = self.controller.send_recv(command);
            match value {
                Ok(SpecialBytes::ACK | SpecialBytes::SELF_TEST_PASSED) => {
                    return value;
                }
                Ok(SpecialBytes::SELF_TEST_FAIL | SpecialBytes::SELF_TEST_FAIL2) => {
                    return Err(Ps2Error::KeyboardReset);
                }
                _ => {}
            }
        }
        self.controller.send_recv(command)
    }

    /// Poll for a scancode from the keyboard, filtering out controller-generated artifacts.
    #[must_use]
    fn read_scancode(&self) -> Option<u8> {
        let status = self.controller.read_status();
        if status & 1 == 0 {
            return None;
        }

        let value = self.controller.read_data();
        match value {
            SpecialBytes::ACK
            | SpecialBytes::RESEND
            | SpecialBytes::SELF_TEST_PASSED
            | SpecialBytes::SELF_TEST_FAIL
            | SpecialBytes::SELF_TEST_FAIL2
            | SpecialBytes::ECHO
            | SpecialBytes::ERROR
            | SpecialBytes::KEY_ERROR => None,
            _ => Some(value),
        }
    }

    /// Flush the keyboard output buffer.
    fn flush_output_buffer(&self) {
        while self.controller.read_status() & 1 != 0 {
            let _ = self.controller.read_data();
        }
    }
}

/// State machine flags for multi-byte PS/2 scancode sequences.
///
/// All state is packed into a single `u16` to fit in an AtomicU16:
/// - Bits [0:3]: Transient flags (extended, release, print_make, print_break)
/// - Bits [8:15]: Pause sequence remaining byte counter
///
/// These flags track intermediate states in multi-byte sequences (Pause/Break: 6-8 bytes,
/// PrintScreen: 4-6 bytes depending on set) and are cleared after each complete key event
/// to prevent cross-contamination between unrelated keypresses.
pub struct KeyboardState;

impl KeyboardState {
    /// State flag: next scancode is part of an extended sequence (0xE0 prefix)
    pub const STATE_EXTENDED: u16 = 0b0000_0001;
    /// State flag: next scancode is a release (break) event (0xF0 prefix in Set 2/3)
    pub const STATE_RELEASE: u16 = 0b0000_0010;
    /// State flag: building PrintScreen make sequence
    pub const STATE_PRINT_MAKE: u16 = 0b0000_0100;
    /// State flag: building PrintScreen release sequence
    pub const STATE_PRINT_BREAK: u16 = 0b0000_1000;
    /// Mask for all transient state flags (should be cleared after each decoded key)
    pub const FLAGS_MASK: u16 = Self::STATE_EXTENDED
        | Self::STATE_RELEASE
        | Self::STATE_PRINT_MAKE
        | Self::STATE_PRINT_BREAK;
    /// Bit position where pause counter starts
    pub const PAUSE_SHIFT: u16 = 8;
    /// Mask to extract/clear pause counter
    pub const PAUSE_MASK: u16 = 0xFF << Self::PAUSE_SHIFT;

    #[must_use]
    #[inline]
    /// Get the number of remaining bytes in the Pause/Break sequence.
    pub const fn pause_remaining(state: u16) -> u8 {
        (state >> Self::PAUSE_SHIFT) as u8
    }

    #[must_use]
    #[inline]
    /// Set the pause sequence counter.
    pub fn set_pause_remaining(state: u16, remaining: u8) -> u16 {
        (state & !Self::PAUSE_MASK) | (u16::from(remaining) << Self::PAUSE_SHIFT)
    }

    #[must_use]
    #[inline]
    /// Check if the extended flag is set.
    pub const fn is_extended(state: u16) -> bool {
        (state & Self::STATE_EXTENDED) != 0
    }

    #[must_use]
    #[inline]
    /// Check if the release flag is set.
    pub const fn is_released(state: u16) -> bool {
        (state & Self::STATE_RELEASE) != 0
    }

    #[must_use]
    #[inline]
    /// Check if PrintScreen make flag is set.
    pub const fn is_print_make(state: u16) -> bool {
        (state & Self::STATE_PRINT_MAKE) != 0
    }

    #[must_use]
    #[inline]
    /// Check if PrintScreen break flag is set.
    pub const fn is_print_break(state: u16) -> bool {
        (state & Self::STATE_PRINT_BREAK) != 0
    }

    #[must_use]
    #[inline]
    /// Clear all state flags except the pause counter.
    pub const fn clear_flags(state: u16) -> u16 {
        state & !0xF
    }

    #[must_use]
    #[inline]
    /// Clear extended and print flags, preserving pause counter.
    pub const fn clear_extended_print(state: u16) -> u16 {
        state & !(Self::STATE_EXTENDED | Self::STATE_PRINT_MAKE | Self::STATE_PRINT_BREAK)
    }

    #[must_use]
    #[inline]
    /// Clear extended and release flags, preserving pause counter.
    pub const fn clear_extended_release(state: u16) -> u16 {
        state & !(Self::STATE_EXTENDED | Self::STATE_RELEASE)
    }

    #[must_use]
    #[inline]
    /// Clear all special flags after processing a key.
    pub const fn clear_all_flags(state: u16) -> u16 {
        state & !Self::FLAGS_MASK
    }

    #[must_use]
    #[inline]
    /// Abort incomplete PrintScreen/PrintBreak sequence and restore clean state.
    /// Used when unexpected bytes are received during sequence decoding.
    pub const fn abort_incomplete_sequence(state: u16) -> u16 {
        state & !(Self::STATE_EXTENDED | Self::STATE_PRINT_MAKE | Self::STATE_PRINT_BREAK)
    }
}
