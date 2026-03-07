use alloc::collections::VecDeque;
use alloc::string::String;

/// Fixed-capacity ring buffer of previous command lines.
pub struct History {
    entries: VecDeque<String>,
    capacity: usize,
    browse_pos: Option<usize>,
    stashed_input: String,
}

impl Default for History {
    fn default() -> Self {
        Self::new()
    }
}

impl History {
    const DEFAULT_CAPACITY: usize = 64;

    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: VecDeque::new(),
            capacity: Self::DEFAULT_CAPACITY,
            browse_pos: None,
            stashed_input: String::new(),
        }
    }

    /// Record a command. Empty or duplicate-of-last entries are ignored.
    pub fn push(&mut self, line: &str) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return;
        }
        if self.entries.back().is_some_and(|last| last == trimmed) {
            return;
        }
        if self.entries.len() >= self.capacity {
            self.entries.pop_front();
        }
        self.entries.push_back(String::from(trimmed));
    }

    /// Begin or continue browsing: move one entry older.
    /// Returns the entry text, or `None` if at the oldest entry.
    pub fn older(&mut self, current_input: &str) -> Option<&str> {
        match self.browse_pos {
            None => {
                if self.entries.is_empty() {
                    return None;
                }
                self.stashed_input = String::from(current_input);
                let idx = self.entries.len() - 1;
                self.browse_pos = Some(idx);
                Some(&self.entries[idx])
            }
            Some(pos) => {
                if pos == 0 {
                    return Some(&self.entries[0]);
                }
                let idx = pos - 1;
                self.browse_pos = Some(idx);
                Some(&self.entries[idx])
            }
        }
    }

    /// Move one entry newer. Returns `None` when returning to the live input.
    pub fn newer(&mut self) -> Option<&str> {
        match self.browse_pos {
            None => None,
            Some(pos) => {
                if pos + 1 >= self.entries.len() {
                    self.browse_pos = None;
                    Some(&self.stashed_input)
                } else {
                    let idx = pos + 1;
                    self.browse_pos = Some(idx);
                    Some(&self.entries[idx])
                }
            }
        }
    }

    /// Cancel browsing (e.g. when the user starts typing).
    pub fn cancel_browse(&mut self) {
        self.browse_pos = None;
        self.stashed_input.clear();
    }
}
