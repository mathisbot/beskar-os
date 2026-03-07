use alloc::vec::Vec;

use super::cell::{Cell, Style};

/// A terminal buffer that stores lines of styled cells with scrollback.
///
/// The viewport is the bottom `rows` lines visible on screen.
/// Scrolling moves the viewport up into history.
pub struct TermBuffer {
    lines: Vec<Vec<Cell>>,
    cols: u16,
    rows: u16,
    cursor_col: u16,
    scroll_offset: usize,
    style: Style,
    dirty: Vec<bool>,
}

impl TermBuffer {
    #[must_use]
    pub fn new(cols: u16, rows: u16) -> Self {
        let mut lines = Vec::with_capacity(rows as usize);
        lines.push(Vec::new());
        let dirty = alloc::vec![true; rows as usize];
        Self {
            lines,
            cols,
            rows,
            cursor_col: 0,
            scroll_offset: 0,
            style: Style::DEFAULT,
            dirty,
        }
    }

    #[must_use]
    #[inline]
    pub const fn cols(&self) -> u16 {
        self.cols
    }

    #[must_use]
    #[inline]
    pub const fn rows(&self) -> u16 {
        self.rows
    }

    #[must_use]
    #[inline]
    pub const fn cursor_col(&self) -> u16 {
        self.cursor_col
    }

    /// The total number of lines (including scrollback).
    #[must_use]
    #[inline]
    pub const fn total_lines(&self) -> usize {
        self.lines.len()
    }

    /// Index of the first visible line.
    const fn viewport_start(&self) -> usize {
        let total = self.lines.len();
        let view_h = self.rows as usize;
        if total <= view_h {
            0
        } else {
            total - view_h - self.scroll_offset
        }
    }

    /// Returns a viewport row as a slice (may be shorter than `cols`).
    #[must_use]
    pub fn viewport_line(&self, row: u16) -> &[Cell] {
        let idx = self.viewport_start() + row as usize;
        self.lines.get(idx).map_or(&[], Vec::as_slice)
    }

    /// The row within the viewport where the cursor sits, if visible.
    #[must_use]
    #[expect(clippy::cast_possible_truncation)]
    pub const fn cursor_viewport_row(&self) -> Option<u16> {
        if self.scroll_offset != 0 {
            return None;
        }
        let total = self.lines.len();
        let view_h = self.rows as usize;
        if total <= view_h {
            Some((total - 1) as u16)
        } else {
            Some(self.rows - 1)
        }
    }

    #[inline]
    pub const fn set_style(&mut self, style: Style) {
        self.style = style;
    }

    #[must_use]
    #[inline]
    pub const fn style_snapshot(&self) -> Style {
        self.style
    }

    /// Write a single character at the cursor, advancing it.
    pub fn put_char(&mut self, ch: char) {
        match ch {
            '\n' => self.newline(),
            '\r' => self.cursor_col = 0,
            '\t' => {
                let spaces = 4 - (self.cursor_col % 4);
                for _ in 0..spaces {
                    self.put_visible(' ');
                }
            }
            c => self.put_visible(c),
        }
    }

    /// Write a string to the buffer.
    pub fn put_str(&mut self, s: &str) {
        for ch in s.chars() {
            self.put_char(ch);
        }
    }

    /// Write a string with a specific style, then restore the previous style.
    pub fn write_styled(&mut self, s: &str, style: Style) {
        let prev = core::mem::replace(&mut self.style, style);
        self.put_str(s);
        self.style = prev;
    }

    /// Clear all content and reset the cursor.
    pub fn clear(&mut self) {
        self.lines.clear();
        self.lines.push(Vec::new());
        self.cursor_col = 0;
        self.scroll_offset = 0;
        self.mark_all_dirty();
    }

    /// Scroll viewport up by `n` lines into history.
    pub fn scroll_up(&mut self, n: usize) {
        let max = self.max_scroll();
        self.scroll_offset = (self.scroll_offset + n).min(max);
        self.mark_all_dirty();
    }

    /// Scroll viewport down by `n` lines toward present.
    pub fn scroll_down(&mut self, n: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(n);
        self.mark_all_dirty();
    }

    /// Snap viewport to the bottom.
    pub fn scroll_to_bottom(&mut self) {
        if self.scroll_offset != 0 {
            self.scroll_offset = 0;
            self.mark_all_dirty();
        }
    }

    /// Check if a viewport row is dirty.
    #[must_use]
    #[inline]
    pub fn is_row_dirty(&self, row: u16) -> bool {
        self.dirty.get(row as usize).copied().unwrap_or(false)
    }

    /// Mark all viewport rows as clean.
    pub fn clear_dirty(&mut self) {
        self.dirty.fill(false);
    }

    /// Mark all viewport rows as dirty.
    pub fn mark_all_dirty(&mut self) {
        self.dirty.fill(true);
    }

    /// Mark only the last `n` content rows (from the cursor upward) as dirty,
    /// clearing all other dirty flags. This is used when only the tail of the
    /// buffer changed (e.g. redrawing a wrapped prompt) and everything above
    /// is guaranteed unchanged.
    pub fn mark_last_n_content_rows_dirty(&mut self, n: usize) {
        self.dirty.fill(false);
        if let Some(cursor_row) = self.cursor_viewport_row() {
            let first = (cursor_row as usize + 1).saturating_sub(n);
            for d in &mut self.dirty[first..=cursor_row as usize] {
                *d = true;
            }
        }
    }

    /// Return the (first, last) dirty viewport row indices, if any.
    #[must_use]
    pub fn dirty_row_range(&self) -> Option<(u16, u16)> {
        let mut first = None;
        let mut last = None;
        for (i, &d) in self.dirty.iter().enumerate() {
            if d {
                #[expect(clippy::cast_possible_truncation)]
                let row = i as u16;
                if first.is_none() {
                    first = Some(row);
                }
                last = Some(row);
            }
        }
        first.zip(last)
    }

    /// Erase the entire current line.
    pub fn erase_line(&mut self) {
        if let Some(line) = self.lines.last_mut() {
            line.clear();
        }
        self.cursor_col = 0;
        self.mark_current_line_dirty();
    }

    /// Remove the last `n` lines, keeping at least one empty line.
    pub fn erase_last_n_lines(&mut self, n: usize) {
        let to_pop = n.saturating_sub(1).min(self.lines.len().saturating_sub(1));
        for _ in 0..to_pop {
            self.lines.pop();
        }
        if let Some(line) = self.lines.last_mut() {
            line.clear();
        }
        self.cursor_col = 0;
        self.mark_all_dirty();
    }

    /// Override the cursor column (clamped to `cols`).
    pub fn set_cursor_col(&mut self, col: u16) {
        self.cursor_col = col.min(self.cols);
    }

    fn put_visible(&mut self, ch: char) {
        if self.cursor_col >= self.cols {
            self.newline();
        }
        let cell = Cell::new(ch, self.style);
        let line = self.lines.last_mut().expect("buffer always has >=1 line");
        let col = self.cursor_col as usize;

        if col < line.len() {
            line[col] = cell;
        } else {
            // Fill gap with blanks if cursor jumped ahead
            while line.len() < col {
                line.push(Cell::BLANK);
            }
            line.push(cell);
        }

        self.cursor_col += 1;
        self.mark_current_line_dirty();
    }

    fn newline(&mut self) {
        self.lines.push(Vec::new());
        self.cursor_col = 0;
        if self.scroll_offset == 0 {
            self.mark_all_dirty();
        }
    }

    const fn max_scroll(&self) -> usize {
        self.lines.len().saturating_sub(self.rows as usize)
    }

    fn mark_current_line_dirty(&mut self) {
        if self.scroll_offset == 0
            && let Some(row) = self.cursor_viewport_row()
            && let Some(d) = self.dirty.get_mut(row as usize)
        {
            *d = true;
        }
    }
}

impl core::fmt::Write for TermBuffer {
    fn write_char(&mut self, c: char) -> core::fmt::Result {
        self.put_char(c);
        Ok(())
    }

    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.put_str(s);
        Ok(())
    }
}
