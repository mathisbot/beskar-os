use alloc::vec::Vec;

use super::builtins::{self, CmdCtx};
use crate::buffer::{Style, TermBuffer};
use crate::input::{EditResult, LineEditor};
use crate::render::Rasterizer;
use crate::theme;

use beskar_lib::io::keyboard::KeyEvent;

const PROMPT_USER: &str = "beskar";
const PROMPT_SEP: &str = "://";
const PROMPT_CHEVRON: &str = "> ";

/// The shell orchestrator: owns buffer, editor, and renderer.
pub struct Shell {
    buf: TermBuffer,
    editor: LineEditor,
    renderer: Rasterizer,
    /// Buffer line index where the current prompt starts.
    prompt_start_line: usize,
    /// Line count at the last status-bar render, to skip redundant redraws.
    last_rendered_line_count: usize,
}

impl Shell {
    /// Boot the shell: allocate surfaces, render welcome banner, show prompt.
    #[must_use]
    #[expect(clippy::new_without_default)]
    pub fn new() -> Self {
        const STATUS_ROWS: u16 = 1;

        let renderer = Rasterizer::new(STATUS_ROWS);
        let cols = renderer.cols();
        let rows = renderer.content_rows();

        let mut buf = TermBuffer::new(cols, rows);

        let accent = Style {
            fg: theme::ACCENT_CYAN,
            bg: theme::BG_PRIMARY,
        };
        let dim = Style {
            fg: theme::FG_DIMMED,
            bg: theme::BG_PRIMARY,
        };
        let rule = Style {
            fg: theme::PROMPT_CHEVRON,
            bg: theme::BG_PRIMARY,
        };

        // Horizontal rule
        let rule_len = (cols as usize).min(56);
        for _ in 0..rule_len {
            buf.write_styled("-", rule);
        }
        buf.put_char('\n');

        buf.write_styled("  BESKAR-OS TERMINAL", accent);
        buf.write_styled("  //  ", dim);
        buf.write_styled("bashkar", accent);
        buf.put_char('\n');

        for _ in 0..rule_len {
            buf.write_styled("-", rule);
        }
        buf.put_char('\n');

        buf.put_char('\n');
        buf.write_styled("  Shell operational. Type ", dim);
        buf.write_styled("help", accent);
        buf.write_styled(" for commands.", dim);
        buf.put_char('\n');
        buf.put_char('\n');

        let mut shell = Self {
            buf,
            editor: LineEditor::new(),
            renderer,
            prompt_start_line: 0,
            last_rendered_line_count: 0,
        };
        shell.prompt_start_line = shell.buf.total_lines().saturating_sub(1);
        shell.write_prompt();
        shell.render();
        shell
    }

    /// Process a keyboard event from the event loop.
    pub fn handle_key(&mut self, event: &KeyEvent) {
        let result = self.editor.handle_key(event);

        match result {
            EditResult::Continue => {
                self.redraw_input_line();
                self.render();
            }
            EditResult::Submit(line) => {
                self.buf.put_char('\n');
                self.execute(&line);
                self.prompt_start_line = self.buf.total_lines().saturating_sub(1);
                self.write_prompt();
                self.render();
            }
            EditResult::Scroll(delta) => {
                let abs_delta = usize::from(delta.unsigned_abs());
                if delta > 0 {
                    self.buf.scroll_up(abs_delta);
                } else {
                    self.buf.scroll_down(abs_delta);
                }
                self.render();
            }
        }
    }

    fn write_prompt(&mut self) {
        let bg = theme::BG_PRIMARY;
        self.buf.write_styled(
            PROMPT_USER,
            Style {
                fg: theme::PROMPT_USER,
                bg,
            },
        );
        self.buf.write_styled(
            PROMPT_SEP,
            Style {
                fg: theme::PROMPT_SEPARATOR,
                bg,
            },
        );
        self.buf.write_styled(
            PROMPT_CHEVRON,
            Style {
                fg: theme::PROMPT_CHEVRON,
                bg,
            },
        );
    }

    /// Erase and rewrite the current prompt + input line.
    fn redraw_input_line(&mut self) {
        self.buf.scroll_to_bottom();

        // How many buffer lines the prompt+input occupies.
        let prev_line_count = self
            .buf
            .total_lines()
            .saturating_sub(self.prompt_start_line);

        if prev_line_count <= 1 {
            self.buf.erase_line();
        } else {
            self.buf.erase_last_n_lines(prev_line_count);
        }

        self.write_prompt();
        let prompt_len = PROMPT_USER.len() + PROMPT_SEP.len() + PROMPT_CHEVRON.len();
        self.buf.put_str(self.editor.line());

        // Cursor column for wrapped input.
        let total_cursor_pos = prompt_len + self.editor.cursor_char_offset();
        let cols = self.buf.cols() as usize;
        #[expect(clippy::cast_possible_truncation)]
        let cursor_col = if cols > 0 {
            (total_cursor_pos % cols) as u16
        } else {
            0
        };
        self.buf.set_cursor_col(cursor_col);

        // Only the prompt rows changed; narrow the dirty set.
        let new_line_count = self
            .buf
            .total_lines()
            .saturating_sub(self.prompt_start_line);
        let affected = prev_line_count.max(new_line_count);
        self.buf.mark_last_n_content_rows_dirty(affected);
    }

    fn execute(&mut self, line: &str) {
        let (cmd, args) = parse_command_line(line);
        if cmd.is_empty() {
            return;
        }

        if let Some(builtin) = builtins::find(cmd) {
            let ctx = CmdCtx {
                buf: &mut self.buf,
                args: &args,
            };
            if let Err(msg) = (builtin.run)(ctx) {
                self.buf.write_styled(
                    "error: ",
                    Style {
                        fg: theme::ACCENT_RED,
                        bg: theme::BG_PRIMARY,
                    },
                );
                self.buf.put_str(&msg);
                self.buf.put_char('\n');
            }
        } else {
            self.buf.write_styled(
                "unknown command: ",
                Style {
                    fg: theme::ACCENT_AMBER,
                    bg: theme::BG_PRIMARY,
                },
            );
            self.buf.put_str(cmd);
            self.buf.put_char('\n');
        }
    }

    fn render(&mut self) {
        let sr = self.renderer.status_rows();
        let line_count = self.buf.total_lines();
        if line_count != self.last_rendered_line_count {
            self.renderer.draw_status_bar("BASHKAR", line_count);
            self.last_rendered_line_count = line_count;
            self.renderer.present_rows(0, sr);
        }

        self.renderer.draw_buffer(&self.buf, true);

        if let Some((first, last)) = self.buf.dirty_row_range() {
            self.renderer.present_rows(sr + first, last - first + 1);
        }

        self.buf.clear_dirty();
    }
}

fn parse_command_line(line: &str) -> (&str, Vec<&str>) {
    let mut parts = line.split_whitespace();
    let cmd = parts.next().unwrap_or("");
    let args: Vec<&str> = parts.collect();
    (cmd, args)
}
