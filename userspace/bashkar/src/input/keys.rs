use beskar_lib::io::keyboard::{KeyCode, KeyEvent, KeyModifiers, KeyState};

/// High-level action derived from a raw key event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Insert(char),
    Submit,
    Backspace,
    Delete,
    CursorLeft,
    CursorRight,
    Home,
    End,
    HistoryOlder,
    HistoryNewer,
    ScrollUp,
    ScrollDown,
    ModifierChange,
    None,
}

/// Translate a `KeyEvent` into an [`Action`], updating `modifiers` in place.
pub fn translate(event: KeyEvent, modifiers: &mut KeyModifiers) -> Action {
    let key = event.key();
    let pressed = event.pressed();

    // Handle modifier keys
    match key {
        KeyCode::ShiftLeft | KeyCode::ShiftRight => {
            modifiers.set_shifted(pressed == KeyState::Pressed);
            return Action::ModifierChange;
        }
        KeyCode::CtrlLeft | KeyCode::CtrlRight => {
            modifiers.set_ctrled(pressed == KeyState::Pressed);
            return Action::ModifierChange;
        }
        KeyCode::AltLeft | KeyCode::AltRight => {
            modifiers.set_alted(pressed == KeyState::Pressed);
            return Action::ModifierChange;
        }
        KeyCode::CapsLock if pressed == KeyState::Pressed => {
            modifiers.set_caps_locked(!modifiers.is_caps_locked());
            return Action::ModifierChange;
        }
        _ => {}
    }

    if pressed != KeyState::Pressed {
        return Action::None;
    }

    if modifiers.is_ctrled() {
        return match key {
            KeyCode::A => Action::Home,
            KeyCode::E => Action::End,
            _ => Action::None,
        };
    }

    match key {
        KeyCode::Enter => Action::Submit,
        KeyCode::Backspace => Action::Backspace,
        KeyCode::Delete => Action::Delete,
        KeyCode::ArrowLeft => Action::CursorLeft,
        KeyCode::ArrowRight => Action::CursorRight,
        KeyCode::ArrowUp => {
            if modifiers.is_shifted() {
                Action::ScrollUp
            } else {
                Action::HistoryOlder
            }
        }
        KeyCode::ArrowDown => {
            if modifiers.is_shifted() {
                Action::ScrollDown
            } else {
                Action::HistoryNewer
            }
        }
        KeyCode::Home => Action::Home,
        KeyCode::End => Action::End,
        KeyCode::PageUp => Action::ScrollUp,
        KeyCode::PageDown => Action::ScrollDown,
        k => {
            let ch = k.as_char(*modifiers);
            if ch == '\0' {
                Action::None
            } else {
                Action::Insert(ch)
            }
        }
    }
}
