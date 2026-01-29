use super::KeyboardState;
use super::keycodes::{Ps2Set1Keycodes, Ps2Set2Keycodes};
use beskar_core::drivers::keyboard::{KeyCode, KeyEvent, KeyState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ScancodeSet {
    Set1 = 1,
    Set2 = 2,
    Set3 = 3,
}

impl TryFrom<u8> for ScancodeSet {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Set1),
            2 => Ok(Self::Set2),
            3 => Ok(Self::Set3),
            _ => Err(()),
        }
    }
}

/// Scancode decoder for PS/2 keyboards supporting all three scancode sets.
///
/// Handles multi-byte sequences (Pause/Break: 6 or 8 bytes, PrintScreen: 4 or 6 bytes)
/// and implicit state machines triggered by prefix bytes (0xE0, 0xF0, 0xE1).
pub struct ScancodeDecoder;

impl ScancodeDecoder {
    /// Decode a single scancode into an optional KeyEvent, advancing multi-byte sequence state.
    ///
    /// Returns `None` if:
    /// - The byte is a prefix (0xE0, 0xF0, 0xE1) that sets state for the next byte(s)
    /// - We are consuming intermediate bytes of a Pause/Break sequence
    /// - We are building a PrintScreen make/break sequence
    ///
    /// Returns `Some(KeyEvent)` when a complete key event is decoded, after which state is reset.
    #[must_use]
    pub fn decode(scancode: u8, state: &mut u16, scancode_set: ScancodeSet) -> Option<KeyEvent> {
        // Handle Pause/Break multi-byte sequence state machine.
        // Pause/Break sequences have fixed length (6 bytes in Set 1, 8 bytes in Sets 2/3).
        // No other key is transmitted during this sequence; all intermediate bytes are consumed.
        // Only the final byte triggers a KeyEvent emit (PauseBreak press with no release).
        let pause_remaining = KeyboardState::pause_remaining(*state);
        if pause_remaining != 0 {
            let new_remaining = pause_remaining - 1;
            // Clear all flags while consuming Pause sequence, preserving only pause counter.
            *state = KeyboardState::set_pause_remaining(
                *state & !KeyboardState::FLAGS_MASK,
                new_remaining,
            );

            let event = if new_remaining == 0 {
                Some(KeyEvent::new(KeyCode::PauseBreak, KeyState::Pressed))
            } else {
                None
            };

            return event;
        }

        // Detect Pause/Break sequence starter (0xE1 in all sets).
        // This is the only multi-byte sequence initiated by 0xE1; all sets recognize it.
        // If we see 0xE1, consume the remaining bytes before emitting the key event.
        if scancode == 0xE1 {
            let remaining = match scancode_set {
                ScancodeSet::Set1 => 5,
                ScancodeSet::Set2 | ScancodeSet::Set3 => 7,
            };
            *state = KeyboardState::set_pause_remaining(*state, remaining);
            return None;
        }

        // Handle extended sequence prefix (0xE0).
        // Sets the STATE_EXTENDED flag so that the next scancode byte is interpreted
        // as an extended key.
        if scancode == 0xE0 {
            *state |= KeyboardState::STATE_EXTENDED;
            return None;
        }

        // Handle release prefix for Set 2 and Set 3 (0xF0).
        // Sets the STATE_RELEASE flag so that the next scancode byte is interpreted
        // as a key release.
        // Set 1 does not use this prefix; release is encoded in bit 7 instead.
        if scancode == 0xF0 && scancode_set != ScancodeSet::Set1 {
            *state |= KeyboardState::STATE_RELEASE;
            return None;
        }

        // Decode the scancode based on the active set.
        match scancode_set {
            ScancodeSet::Set1 => Self::decode_set1(state, scancode),
            ScancodeSet::Set2 | ScancodeSet::Set3 => Self::decode_set2_like(state, scancode),
        }
    }

    #[must_use]
    /// Decode Set 1 scancodes with hardware-specific handling.
    fn decode_set1(state: &mut u16, mut scancode: u8) -> Option<KeyEvent> {
        let extended = KeyboardState::is_extended(*state);
        let in_print_make = KeyboardState::is_print_make(*state);
        let in_print_break = KeyboardState::is_print_break(*state);

        // Handle PrintScreen make sequence: E0 2A E0 37
        // STATE_PRINT_MAKE tracks the intermediate step (E0 2A detected, waiting for E0 37).
        // If we see E0 2A, switch to PRINT_MAKE state and consume this pair without emitting.
        if extended && scancode == 0x2A && !in_print_make && !in_print_break {
            *state = (*state & !KeyboardState::STATE_EXTENDED) | KeyboardState::STATE_PRINT_MAKE;
            return None;
        }

        // Emit PrintScreen press when we complete the make sequence.
        // We have consumed all 4 bytes: E0 2A E0 37. Emit the key event and clear all flags.
        if extended && scancode == 0x37 && in_print_make {
            *state = KeyboardState::clear_all_flags(*state);
            return Some(KeyEvent::new(KeyCode::PrintScreen, KeyState::Pressed));
        }

        // Handle PrintScreen break sequence: E0 B7 E0 AA
        // STATE_PRINT_BREAK tracks the intermediate step (E0 B7 detected, waiting for E0 AA).
        // If we see E0 B7, switch to PRINT_BREAK state and consume this pair without emitting.
        if extended && scancode == 0xB7 && !in_print_make && !in_print_break {
            *state = (*state & !KeyboardState::STATE_EXTENDED) | KeyboardState::STATE_PRINT_BREAK;
            return None;
        }

        // Emit PrintScreen release when we complete the break sequence.
        // We have consumed all 4 bytes: E0 B7 E0 AA. Emit the key event and clear all flags.
        if extended && scancode == 0xAA && in_print_break {
            *state = KeyboardState::clear_all_flags(*state);
            return Some(KeyEvent::new(KeyCode::PrintScreen, KeyState::Released));
        }

        // If we were building a PrintScreen sequence but got an unexpected byte,
        // abort the sequence to prevent state corruption.
        if in_print_make || in_print_break {
            *state = KeyboardState::abort_incomplete_sequence(*state);
            return None;
        }

        // Determine key state from bit 7 of the scancode.
        // Set 1 encodes release by setting bit 7: scancode = 0x80 | make_code.
        let pressed = if scancode & 0x80 == 0 {
            KeyState::Pressed
        } else {
            scancode &= 0x7F;
            KeyState::Released
        };

        *state = KeyboardState::clear_all_flags(*state);

        let keycode = Ps2Set1Keycodes::map(extended, scancode)?;
        Some(KeyEvent::new(keycode, pressed))
    }

    /// Decode Set 2 or Set 3 scancodes (functionally identical for decode purposes).
    #[must_use]
    fn decode_set2_like(state: &mut u16, scancode: u8) -> Option<KeyEvent> {
        let extended = KeyboardState::is_extended(*state);
        let released = KeyboardState::is_released(*state);
        let in_print_make = KeyboardState::is_print_make(*state);
        let in_print_break = KeyboardState::is_print_break(*state);

        // Handle PrintScreen make sequence: E0 12 E0 7C
        // STATE_PRINT_MAKE tracks the intermediate step (E0 12 detected, waiting for E0 7C).
        // If we see E0 12, switch to PRINT_MAKE state and consume this pair without emitting.
        if extended && !released && scancode == 0x12 && !in_print_make && !in_print_break {
            *state = (*state & !(KeyboardState::STATE_EXTENDED | KeyboardState::STATE_RELEASE))
                | KeyboardState::STATE_PRINT_MAKE;
            return None;
        }

        // Emit PrintScreen press when we complete the make sequence.
        // We have consumed all 4 bytes: E0 12 E0 7C. Emit the key event and clear all flags.
        if extended && !released && scancode == 0x7C && in_print_make {
            *state = KeyboardState::clear_all_flags(*state);
            return Some(KeyEvent::new(KeyCode::PrintScreen, KeyState::Pressed));
        }

        // Handle PrintScreen break sequence: E0 F0 7C E0 F0 12
        // STATE_PRINT_BREAK tracks the first intermediate step (E0 F0 7C detected, waiting for E0 F0 12).
        // If we see E0 F0 7C, switch to PRINT_BREAK state and consume this pair without emitting.
        if extended && released && scancode == 0x7C && !in_print_make && !in_print_break {
            *state = (*state & !(KeyboardState::STATE_EXTENDED | KeyboardState::STATE_RELEASE))
                | KeyboardState::STATE_PRINT_BREAK;
            return None;
        }

        // Emit PrintScreen release when we complete the break sequence.
        // We have consumed all 6 bytes: E0 F0 7C E0 F0 12. Emit the key event and clear all flags.
        if extended && released && scancode == 0x12 && in_print_break {
            *state = KeyboardState::clear_all_flags(*state);
            return Some(KeyEvent::new(KeyCode::PrintScreen, KeyState::Released));
        }

        // If we were building a PrintScreen sequence but got an unexpected byte,
        // abort the sequence to prevent state corruption.
        if in_print_make || in_print_break {
            *state = KeyboardState::abort_incomplete_sequence(*state);
            return None;
        }

        // Determine key state from the release flag.
        // Sets 2 and 3 use an 0xF0 prefix to indicate release; without it, the key is pressed.
        let pressed = if released {
            KeyState::Released
        } else {
            KeyState::Pressed
        };

        *state = KeyboardState::clear_all_flags(*state);

        let keycode = Ps2Set2Keycodes::map(extended, scancode)?;
        Some(KeyEvent::new(keycode, pressed))
    }
}
