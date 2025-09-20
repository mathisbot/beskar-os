use num_enum::{FromPrimitive, IntoPrimitive, TryFromPrimitive};

#[derive(Debug, Clone, Copy)]
pub struct KeyEvent {
    key: KeyCode,
    pressed: KeyState,
}

impl KeyEvent {
    /// The value used to represent `None` when packing the key event.
    ///
    /// This value MUST NOT represent a valid key event.
    const NONE: u64 = 0xFFFF;

    #[must_use]
    #[inline]
    pub const fn new(key: KeyCode, pressed: KeyState) -> Self {
        Self { key, pressed }
    }

    #[must_use]
    #[inline]
    pub const fn key(&self) -> KeyCode {
        self.key
    }

    #[must_use]
    #[inline]
    pub const fn pressed(&self) -> KeyState {
        self.pressed
    }

    #[must_use]
    #[inline]
    pub fn pack_option(key_event: Option<Self>) -> u64 {
        key_event.map_or(Self::NONE, |event| {
            let key = u64::from(<KeyCode as Into<u8>>::into(event.key()));
            let pressed = u64::from(<KeyState as Into<u8>>::into(event.pressed()));
            key | (pressed << 8)
        })
    }

    #[must_use]
    #[inline]
    pub fn unpack_option(value: u64) -> Option<Self> {
        let key = KeyCode::from(u8::try_from(value & 0xFF).ok()?);
        let pressed = KeyState::try_from(u8::try_from((value >> 8) & 0xFF).unwrap()).ok()?;
        Some(Self { key, pressed })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, FromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum KeyCode {
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,

    Num0,
    Num1,
    Num2,
    Num3,
    Num4,
    Num5,
    Num6,
    Num7,
    Num8,
    Num9,

    Enter,
    Escape,
    Backspace,
    Tab,
    Space,
    ShiftLeft,
    ShiftRight,
    CtrlLeft,
    CtrlRight,
    AltLeft,
    AltRight,
    CapsLock,

    Insert,
    Delete,
    Home,
    End,
    PageUp,
    PageDown,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,

    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,

    NumLock,
    Numpad0,
    Numpad1,
    Numpad2,
    Numpad3,
    Numpad4,
    Numpad5,
    Numpad6,
    Numpad7,
    Numpad8,
    Numpad9,
    NumpadAdd,
    NumpadSub,
    NumpadMul,
    NumpadDiv,
    NumpadEnter,
    NumpadDot,

    Minus,
    Equal,
    LeftBracket,
    RightBracket,
    Backslash,
    Semicolon,
    Apostrophe,
    Tilde,
    Comma,
    Dot,
    Slash,

    PrintScreen,
    ScrollLock,
    PauseBreak,
    Menu,
    WindowsLeft,
    WindowsRight,

    #[num_enum(default)]
    Unknown,
}

impl KeyCode {
    #[must_use]
    #[inline]
    pub const fn is_numpad(&self) -> bool {
        matches!(
            self,
            Self::Numpad0
                | Self::Numpad1
                | Self::Numpad2
                | Self::Numpad3
                | Self::Numpad4
                | Self::Numpad5
                | Self::Numpad6
                | Self::Numpad7
                | Self::Numpad8
                | Self::Numpad9
                | Self::NumpadAdd
                | Self::NumpadSub
                | Self::NumpadMul
                | Self::NumpadDiv
                | Self::NumpadEnter
                | Self::NumpadDot
        )
    }

    #[must_use]
    pub const fn as_char(&self, modifiers: KeyModifiers) -> char {
        let raw = match self {
            Self::A => 'a',
            Self::B => 'b',
            Self::C => 'c',
            Self::D => 'd',
            Self::E => 'e',
            Self::F => 'f',
            Self::G => 'g',
            Self::H => 'h',
            Self::I => 'i',
            Self::J => 'j',
            Self::K => 'k',
            Self::L => 'l',
            Self::M => 'm',
            Self::N => 'n',
            Self::O => 'o',
            Self::P => 'p',
            Self::Q => 'q',
            Self::R => 'r',
            Self::S => 's',
            Self::T => 't',
            Self::U => 'u',
            Self::V => 'v',
            Self::W => 'w',
            Self::X => 'x',
            Self::Y => 'y',
            Self::Z => 'z',
            Self::Space => ' ',
            Self::Numpad0 => '0',
            Self::Numpad1 => '1',
            Self::Numpad2 => '2',
            Self::Numpad3 => '3',
            Self::Numpad4 => '4',
            Self::Numpad5 => '5',
            Self::Numpad6 => '6',
            Self::Numpad7 => '7',
            Self::Numpad8 => '8',
            Self::Numpad9 => '9',
            _ => '\0',
        };

        if raw.is_ascii_alphabetic() && modifiers.is_uppercase() {
            raw.to_ascii_uppercase()
        } else {
            raw
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum KeyState {
    Pressed,
    Released,
}

#[derive(Default, Debug, Clone, Copy)]
pub struct KeyModifiers {
    flags: u8,
}

impl KeyModifiers {
    const SHIFT: u8 = 0b0000_0001;
    const CTRL: u8 = 0b0000_0010;
    const ALT: u8 = 0b0000_0100;
    const CAPS_LOCK: u8 = 0b0000_1000;
    const NUM_LOCK: u8 = 0b0001_0000;

    #[must_use]
    #[inline]
    pub const fn new() -> Self {
        Self { flags: 0 }
    }

    #[must_use]
    #[inline]
    pub const fn is_shifted(&self) -> bool {
        self.flags & Self::SHIFT != 0
    }

    #[must_use]
    #[inline]
    pub const fn is_ctrled(&self) -> bool {
        self.flags & Self::CTRL != 0
    }

    #[must_use]
    #[inline]
    pub const fn is_alted(&self) -> bool {
        self.flags & Self::ALT != 0
    }

    #[must_use]
    #[inline]
    pub const fn is_caps_locked(&self) -> bool {
        self.flags & Self::CAPS_LOCK != 0
    }

    #[must_use]
    #[inline]
    pub const fn is_num_locked(&self) -> bool {
        self.flags & Self::NUM_LOCK != 0
    }

    #[must_use]
    #[inline]
    pub const fn is_uppercase(&self) -> bool {
        self.is_shifted() ^ self.is_caps_locked()
    }

    #[inline]
    pub const fn set_shifted(&mut self, shifted: bool) {
        if shifted {
            self.flags |= Self::SHIFT;
        } else {
            self.flags &= !Self::SHIFT;
        }
    }

    #[inline]
    pub const fn set_ctrled(&mut self, ctrled: bool) {
        if ctrled {
            self.flags |= Self::CTRL;
        } else {
            self.flags &= !Self::CTRL;
        }
    }

    #[inline]
    pub const fn set_alted(&mut self, alted: bool) {
        if alted {
            self.flags |= Self::ALT;
        } else {
            self.flags &= !Self::ALT;
        }
    }

    #[inline]
    pub const fn set_caps_locked(&mut self, caps_locked: bool) {
        if caps_locked {
            self.flags |= Self::CAPS_LOCK;
        } else {
            self.flags &= !Self::CAPS_LOCK;
        }
    }

    #[inline]
    pub const fn set_num_locked(&mut self, num_locked: bool) {
        if num_locked {
            self.flags |= Self::NUM_LOCK;
        } else {
            self.flags &= !Self::NUM_LOCK;
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_keycode_packing() {
        let key_event = super::KeyEvent::new(super::KeyCode::A, super::KeyState::Pressed);
        let packed = super::KeyEvent::pack_option(Some(key_event));
        let unpacked = super::KeyEvent::unpack_option(packed).unwrap();
        assert_eq!(key_event.key(), unpacked.key());
        assert_eq!(key_event.pressed(), unpacked.pressed());

        let none_packed = super::KeyEvent::pack_option(None);
        assert_eq!(none_packed, super::KeyEvent::NONE);
        let none_unpacked = super::KeyEvent::unpack_option(none_packed);
        assert!(none_unpacked.is_none());
    }

    #[test]
    fn test_keycode_casing() {
        let mut modifiers = super::KeyModifiers::new();

        assert!(!modifiers.is_uppercase());
        assert_eq!(super::KeyCode::A.as_char(modifiers), 'a');
        assert_eq!(super::KeyCode::Z.as_char(modifiers), 'z');
        assert_eq!(super::KeyCode::Space.as_char(modifiers), ' ');

        modifiers.set_caps_locked(true);
        assert!(modifiers.is_uppercase());
        assert_eq!(super::KeyCode::A.as_char(modifiers), 'A');
        assert_eq!(super::KeyCode::Z.as_char(modifiers), 'Z');
        assert_eq!(super::KeyCode::Space.as_char(modifiers), ' ');

        modifiers.set_shifted(true);
        assert!(!modifiers.is_uppercase());
        assert_eq!(super::KeyCode::A.as_char(modifiers), 'a');
        assert_eq!(super::KeyCode::Z.as_char(modifiers), 'z');
        assert_eq!(super::KeyCode::Space.as_char(modifiers), ' ');
    }
}
