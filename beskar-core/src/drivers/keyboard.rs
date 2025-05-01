use num_enum::{IntoPrimitive, TryFromPrimitive};

#[derive(Debug, Clone, Copy)]
pub struct KeyEvent {
    key: KeyCode,
    pressed: KeyState,
}

impl KeyEvent {
    /// The value used to represent `None` when packing the key event.
    ///
    /// This value MUST NOT represent a valid key event.
    const NONE: u64 = u64::MAX;

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
    pub const fn stub() -> Self {
        Self {
            key: KeyCode::Unknown,
            pressed: KeyState::Released,
        }
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
        if value == Self::NONE {
            None
        } else {
            debug_assert!(value >> 16 == 0);
            let key = KeyCode::try_from(u8::try_from(value & 0xFF).unwrap()).unwrap();
            let pressed = KeyState::try_from(u8::try_from((value >> 8) & 0xFF).unwrap()).unwrap();
            Some(Self { key, pressed })
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
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

    Unknown,
}

impl KeyCode {
    #[must_use]
    pub const fn as_char(&self) -> char {
        match self {
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

            _ => '\0',
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum KeyState {
    Pressed,
    Released,
}
