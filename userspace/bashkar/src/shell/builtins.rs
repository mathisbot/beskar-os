use alloc::string::{String, ToString as _};
use core::fmt::Write as _;

use beskar_core::video::PixelComponents;

use crate::buffer::{Style, TermBuffer};
use crate::theme;

/// Context passed to every builtin command.
pub struct CmdCtx<'a> {
    pub buf: &'a mut TermBuffer,
    pub args: &'a [&'a str],
}

pub type CmdResult = Result<(), String>;

pub struct Builtin {
    pub name: &'static str,
    pub summary: &'static str,
    pub run: fn(CmdCtx<'_>) -> CmdResult,
}

/// All built-in commands.
pub static BUILTINS: &[Builtin] = &[
    Builtin {
        name: "help",
        summary: "List available commands",
        run: cmd_help,
    },
    Builtin {
        name: "echo",
        summary: "Print arguments to the terminal",
        run: cmd_echo,
    },
    Builtin {
        name: "clear",
        summary: "Clear the terminal screen",
        run: cmd_clear,
    },
    Builtin {
        name: "rand",
        summary: "Generate random bytes (usage: rand [n])",
        run: cmd_rand,
    },
    Builtin {
        name: "uptime",
        summary: "Show system uptime",
        run: cmd_uptime,
    },
    Builtin {
        name: "exit",
        summary: "Exit the shell",
        run: cmd_exit,
    },
    Builtin {
        name: "shutdown",
        summary: "Shutdown the system",
        run: cmd_shutdown,
    },
    Builtin {
        name: "reboot",
        summary: "Reboot the system",
        run: cmd_reboot,
    },
];

pub fn find(name: &str) -> Option<&'static Builtin> {
    BUILTINS.iter().find(|b| b.name == name)
}

const fn styled(fg: PixelComponents) -> Style {
    Style {
        fg,
        bg: theme::BG_PRIMARY,
    }
}

#[expect(clippy::unnecessary_wraps, clippy::needless_pass_by_value)]
fn cmd_help(ctx: CmdCtx<'_>) -> CmdResult {
    ctx.buf
        .write_styled("BASHKAR SHELL", styled(theme::ACCENT_CYAN));
    ctx.buf.put_str(" ");
    ctx.buf
        .write_styled("// available commands\n", styled(theme::FG_DIMMED));

    for b in BUILTINS {
        ctx.buf.put_str("  ");
        ctx.buf.write_styled(b.name, styled(theme::ACCENT_CYAN));
        let pad = 14usize.saturating_sub(b.name.len());
        for _ in 0..pad {
            ctx.buf.put_char(' ');
        }
        ctx.buf.write_styled(b.summary, styled(theme::FG_DIMMED));
        ctx.buf.put_char('\n');
    }
    Ok(())
}

#[expect(clippy::unnecessary_wraps, clippy::needless_pass_by_value)]
fn cmd_echo(ctx: CmdCtx<'_>) -> CmdResult {
    if !ctx.args.is_empty() {
        let output = ctx.args.join(" ");
        ctx.buf.put_str(&output);
    }
    ctx.buf.put_char('\n');
    Ok(())
}

#[expect(clippy::unnecessary_wraps, clippy::needless_pass_by_value)]
fn cmd_clear(ctx: CmdCtx<'_>) -> CmdResult {
    ctx.buf.clear();
    Ok(())
}

#[expect(clippy::needless_pass_by_value)]
fn cmd_rand(ctx: CmdCtx<'_>) -> CmdResult {
    const DEFAULT_BYTES: usize = 32;
    const MAX_BYTES: usize = 65536;

    let n = if let Some(arg) = ctx.args.first() {
        arg.parse::<usize>()
            .map_err(|_| String::from("expected a number"))?
    } else {
        DEFAULT_BYTES
    };

    if n == 0 || n > MAX_BYTES {
        return Err(alloc::format!("count must be 1..={MAX_BYTES}"));
    }

    let mut data = alloc::vec![0u8; n];
    beskar_lib::rand::rand_fill(&mut data)
        .map_err(|e| alloc::format!("random generation failed: {e:?}"))?;

    let hex_style = styled(theme::ACCENT_CYAN);
    let bytes_per_line = (ctx.buf.cols() as usize / 3).max(1);

    for (i, byte) in data.iter().enumerate() {
        if i > 0 && i % bytes_per_line == 0 {
            ctx.buf.put_char('\n');
        }
        let prev = ctx.buf.style_snapshot();
        ctx.buf.set_style(hex_style);
        ctx.buf.put_char(HEX_CHARS[(byte >> 4) as usize] as char);
        ctx.buf.put_char(HEX_CHARS[(byte & 0x0F) as usize] as char);
        ctx.buf.put_char(' ');
        ctx.buf.set_style(prev);
    }
    ctx.buf.put_char('\n');
    Ok(())
}

const HEX_CHARS: [u8; 16] = *b"0123456789ABCDEF";

#[expect(clippy::unnecessary_wraps, clippy::needless_pass_by_value)]
fn cmd_uptime(ctx: CmdCtx<'_>) -> CmdResult {
    let now = beskar_lib::time::now();
    let secs = now.total_millis() / 1000;
    let mins = secs / 60;
    let hours = mins / 60;

    let mut s = alloc::string::String::new();
    let _ = write!(s, "{}h {:02}m {:02}s", hours, mins % 60, secs % 60);
    ctx.buf.write_styled(&s, styled(theme::ACCENT_CYAN));
    ctx.buf.put_char('\n');
    Ok(())
}

#[inline]
fn cmd_exit(_ctx: CmdCtx<'_>) -> CmdResult {
    beskar_lib::exit(beskar_lib::ExitCode::Success);
}

#[inline]
fn cmd_shutdown(_ctx: CmdCtx<'_>) -> CmdResult {
    beskar_lib::power::shutdown();
    Err("This process does not have permission to shutdown the system".to_string())
}

fn cmd_reboot(_ctx: CmdCtx<'_>) -> CmdResult {
    beskar_lib::power::shutdown();
    Err("This process does not have permission to reboot the system".to_string())
}
