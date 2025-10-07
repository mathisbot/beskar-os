use beskar_lib::io::{
    File,
    keyboard::{KeyCode, KeyEvent, KeyState},
};
use hyperdrive::once::Once;

#[link(name = "puredoom", kind = "static")]
unsafe extern "C" {
    unsafe fn doom_key_down(key: DoomKeyT);
    unsafe fn doom_key_up(key: DoomKeyT);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
#[allow(dead_code)]
enum DoomKeyT {
    Unknown = -1,
    Tab = 9,
    Enter = 13,
    Escape = 27,
    Space = 32,
    Apostrophe = b'\'' as _,
    Multiply = b'*' as _,
    Comma = b',' as _,
    Minus = 0x2D,
    Period = b'.' as _,
    Slash = b'/' as _,
    Zero = b'0' as _,
    One = b'1' as _,
    Two = b'2' as _,
    Three = b'3' as _,
    Four = b'4' as _,
    Five = b'5' as _,
    Six = b'6' as _,
    Seven = b'7' as _,
    Eight = b'8' as _,
    Nine = b'9' as _,
    Semicolon = b';' as _,
    Equals = b'=' as _,
    LeftBracket = b'[' as _,
    RightBracket = b']' as _,
    A = b'a' as _,
    B = b'b' as _,
    C = b'c' as _,
    D = b'd' as _,
    E = b'e' as _,
    F = b'f' as _,
    G = b'g' as _,
    H = b'h' as _,
    I = b'i' as _,
    J = b'j' as _,
    K = b'k' as _,
    L = b'l' as _,
    M = b'm' as _,
    N = b'n' as _,
    O = b'o' as _,
    P = b'p' as _,
    Q = b'q' as _,
    R = b'r' as _,
    S = b's' as _,
    T = b't' as _,
    U = b'u' as _,
    V = b'v' as _,
    W = b'w' as _,
    X = b'x' as _,
    Y = b'y' as _,
    Z = b'z' as _,
    Backspace = 127,
    Ctrl = 0x80 + 0x1D,
    LeftArrow = 0xAC,
    UpArrow = 0xAD,
    RightArrow = 0xAE,
    DownArrow = 0xAF,
    Shift = 0x80 + 0x36,
    Alt = 0x80 + 0x38,
    F1 = 0x80 + 0x3B,
    F2 = 0x80 + 0x3C,
    F3 = 0x80 + 0x3D,
    F4 = 0x80 + 0x3E,
    F5 = 0x80 + 0x3F,
    F6 = 0x80 + 0x40,
    F7 = 0x80 + 0x41,
    F8 = 0x80 + 0x42,
    F9 = 0x80 + 0x43,
    F10 = 0x80 + 0x44,
    F11 = 0x80 + 0x57,
    F12 = 0x80 + 0x58,
    Pause = 0xFF,
}

impl From<KeyCode> for DoomKeyT {
    fn from(key: KeyCode) -> Self {
        match key {
            KeyCode::Tab => Self::Tab,
            KeyCode::Enter => Self::Enter,
            KeyCode::Escape => Self::Escape,
            KeyCode::Space => Self::Space,
            KeyCode::Apostrophe => Self::Apostrophe,
            KeyCode::NumpadMul => Self::Multiply,
            KeyCode::Comma => Self::Comma,
            KeyCode::Minus => Self::Minus,
            // KeyCode::Period => Self::Period,
            KeyCode::Slash => Self::Slash,
            KeyCode::Num0 | KeyCode::Numpad0 => Self::Zero,
            KeyCode::Num1 | KeyCode::Numpad1 => Self::One,
            KeyCode::Num2 | KeyCode::Numpad2 => Self::Two,
            KeyCode::Num3 | KeyCode::Numpad3 => Self::Three,
            KeyCode::Num4 | KeyCode::Numpad4 => Self::Four,
            KeyCode::Num5 | KeyCode::Numpad5 => Self::Five,
            KeyCode::Num6 | KeyCode::Numpad6 => Self::Six,
            KeyCode::Num7 | KeyCode::Numpad7 => Self::Seven,
            KeyCode::Num8 | KeyCode::Numpad8 => Self::Eight,
            KeyCode::Num9 | KeyCode::Numpad9 => Self::Nine,
            KeyCode::Semicolon => Self::Semicolon,
            // KeyCode::Equals => Self::Equals,
            KeyCode::LeftBracket => Self::LeftBracket,
            KeyCode::RightBracket => Self::RightBracket,
            KeyCode::A => Self::A,
            KeyCode::B => Self::B,
            KeyCode::C => Self::C,
            KeyCode::D => Self::D,
            KeyCode::E => Self::E,
            KeyCode::F => Self::F,
            KeyCode::G => Self::G,
            KeyCode::H => Self::H,
            KeyCode::I => Self::I,
            KeyCode::J => Self::J,
            KeyCode::K => Self::K,
            KeyCode::L => Self::L,
            KeyCode::M => Self::M,
            KeyCode::N => Self::N,
            KeyCode::O => Self::O,
            KeyCode::P => Self::P,
            KeyCode::Q => Self::Q,
            KeyCode::R => Self::R,
            KeyCode::S => Self::S,
            KeyCode::T => Self::T,
            KeyCode::U => Self::U,
            KeyCode::V => Self::V,
            KeyCode::W => Self::W,
            KeyCode::X => Self::X,
            KeyCode::Y => Self::Y,
            KeyCode::Z => Self::Z,
            KeyCode::Backspace => Self::Backspace,
            KeyCode::CtrlLeft | KeyCode::CtrlRight => Self::Ctrl,
            KeyCode::ArrowLeft => Self::LeftArrow,
            KeyCode::ArrowUp => Self::UpArrow,
            KeyCode::ArrowRight => Self::RightArrow,
            KeyCode::ArrowDown => Self::DownArrow,
            KeyCode::ShiftLeft | KeyCode::ShiftRight => Self::Shift,
            KeyCode::AltLeft | KeyCode::AltRight => Self::Alt,
            KeyCode::F1 => Self::F1,
            KeyCode::F2 => Self::F2,
            KeyCode::F3 => Self::F3,
            KeyCode::F4 => Self::F4,
            KeyCode::F5 => Self::F5,
            KeyCode::F6 => Self::F6,
            KeyCode::F7 => Self::F7,
            KeyCode::F8 => Self::F8,
            KeyCode::F9 => Self::F9,
            KeyCode::F10 => Self::F10,
            KeyCode::F11 => Self::F11,
            KeyCode::F12 => Self::F12,
            KeyCode::PauseBreak => Self::Pause,
            _ => Self::Unknown,
        }
    }
}

#[repr(align(8))]
struct KeyboardEventBuffer([u8; size_of::<u64>()]);

// Capture the keyboard file for the lifetime of the program.
static KEYBOARD_HANDLE: Once<File> = Once::uninit();

#[must_use]
#[inline]
/// # Panics
///
/// This function panics if opening the keyboard file fails (only once).
fn poll_keyboard() -> Option<KeyEvent> {
    const KEYBOARD_FILE: &str = "/dev/keyboard";

    KEYBOARD_HANDLE.call_once(|| File::open(KEYBOARD_FILE).unwrap());
    let file = KEYBOARD_HANDLE.get().unwrap();

    let mut buffer = KeyboardEventBuffer([0_u8; size_of::<u64>()]);
    let bytes_read = file.read(&mut buffer.0, 0).unwrap_or(0);

    if bytes_read == buffer.0.len() {
        let value = u64::from_ne_bytes(buffer.0);
        KeyEvent::unpack_option(value)
    } else {
        None
    }
}

/// Polls the keyboard and redistributes events to Doom.
pub fn poll_inputs() {
    while let Some(event) = poll_keyboard() {
        let doom_key = DoomKeyT::from(event.key());
        match event.pressed() {
            KeyState::Pressed => unsafe { doom_key_down(doom_key) },
            KeyState::Released => unsafe { doom_key_up(doom_key) },
        }
    }
}
