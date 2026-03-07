use alloc::string::String;

use super::history::History;
use super::keys::{self, Action};
use beskar_lib::io::keyboard::{KeyEvent, KeyModifiers};

/// Outcome of processing a key event.
pub enum EditResult {
    /// Line still being edited.
    Continue,
    /// User pressed Enter; contains the submitted line.
    Submit(String),
    /// Viewport scroll (positive = up, negative = down).
    Scroll(i16),
}

/// A readline-style line editor with cursor movement and history.
pub struct LineEditor {
    buf: String,
    cursor: usize,
    /// Character count from start of buf to cursor (kept in sync for O(1) lookup).
    cursor_chars: usize,
    history: History,
    modifiers: KeyModifiers,
}

impl Default for LineEditor {
    fn default() -> Self {
        Self::new()
    }
}

impl LineEditor {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            buf: String::new(),
            cursor: 0,
            cursor_chars: 0,
            history: History::new(),
            modifiers: KeyModifiers::new(),
        }
    }

    /// Process a key event and return what the shell should do.
    pub fn handle_key(&mut self, event: &KeyEvent) -> EditResult {
        let action = keys::translate(*event, &mut self.modifiers);

        match action {
            Action::Insert(ch) => {
                self.history.cancel_browse();
                self.buf.insert(self.cursor, ch);
                self.cursor += ch.len_utf8();
                self.cursor_chars += 1;
                EditResult::Continue
            }
            Action::Submit => {
                let line = core::mem::take(&mut self.buf);
                self.history.push(&line);
                self.history.cancel_browse();
                self.cursor = 0;
                self.cursor_chars = 0;
                EditResult::Submit(line)
            }
            Action::Backspace => {
                if self.cursor > 0 {
                    let prev = self.buf[..self.cursor]
                        .char_indices()
                        .next_back()
                        .map_or(0, |(i, _)| i);
                    self.buf.drain(prev..self.cursor);
                    self.cursor = prev;
                    self.cursor_chars -= 1;
                }
                EditResult::Continue
            }
            Action::Delete => {
                if self.cursor < self.buf.len() {
                    let next = self.buf[self.cursor..]
                        .char_indices()
                        .nth(1)
                        .map_or(self.buf.len(), |(i, _)| self.cursor + i);
                    self.buf.drain(self.cursor..next);
                }
                EditResult::Continue
            }
            Action::CursorLeft => {
                if self.cursor > 0 {
                    self.cursor = self.buf[..self.cursor]
                        .char_indices()
                        .next_back()
                        .map_or(0, |(i, _)| i);
                    self.cursor_chars -= 1;
                }
                EditResult::Continue
            }
            Action::CursorRight => {
                if self.cursor < self.buf.len() {
                    self.cursor = self.buf[self.cursor..]
                        .char_indices()
                        .nth(1)
                        .map_or(self.buf.len(), |(i, _)| self.cursor + i);
                    self.cursor_chars += 1;
                }
                EditResult::Continue
            }
            Action::Home => {
                self.cursor = 0;
                self.cursor_chars = 0;
                EditResult::Continue
            }
            Action::End => {
                self.cursor = self.buf.len();
                self.cursor_chars = self.buf.chars().count();
                EditResult::Continue
            }
            Action::HistoryOlder => {
                if let Some(entry) = self.history.older(&self.buf) {
                    self.buf = String::from(entry);
                    self.cursor = self.buf.len();
                    self.cursor_chars = self.buf.chars().count();
                }
                EditResult::Continue
            }
            Action::HistoryNewer => {
                if let Some(entry) = self.history.newer() {
                    self.buf = String::from(entry);
                    self.cursor = self.buf.len();
                    self.cursor_chars = self.buf.chars().count();
                }
                EditResult::Continue
            }
            Action::ScrollUp => EditResult::Scroll(5),
            Action::ScrollDown => EditResult::Scroll(-5),
            Action::ModifierChange | Action::None => EditResult::Continue,
        }
    }

    /// The current input line.
    #[must_use]
    #[inline]
    pub fn line(&self) -> &str {
        &self.buf
    }

    /// Current cursor position as a character offset.
    #[must_use]
    #[inline]
    pub const fn cursor_char_offset(&self) -> usize {
        self.cursor_chars
    }

    /// Clear the input line.
    pub fn clear(&mut self) {
        self.buf.clear();
        self.cursor = 0;
        self.cursor_chars = 0;
    }
}
