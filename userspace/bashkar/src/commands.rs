//! Shell command implementations
use crate::video::tty::Tty;
use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use core::fmt::Write as _;

/// A shell command result
pub type CommandResult = Result<(), String>;

/// Execute a command with its arguments
///
/// # Errors
///
/// Returns `Ok(())` if the command was executed successfully.
/// Returns `Err(String)` if the command was not recognized or failed.
pub fn execute_command(command: &str, args: &[String], tty: &mut Tty) -> CommandResult {
    match command {
        "" => Ok(()),
        "help" => {
            cmd_help(tty);
            Ok(())
        }
        "echo" => {
            cmd_echo(args, tty);
            Ok(())
        }
        "clear" => {
            cmd_clear(tty);
            Ok(())
        }
        "exit" => beskar_lib::exit(beskar_lib::ExitCode::Success),
        "rand" => cmd_rand(args, tty),
        _ => Err(alloc::format!("Unknown command: {command}")),
    }
}

/// Parse a command line into a command and arguments
pub fn parse_command_line(line: &str) -> (String, Vec<String>) {
    let mut parts = line.split_whitespace();
    let command = parts.next().unwrap_or("").to_string();
    let args = parts.map(ToString::to_string).collect();
    (command, args)
}

/// Display help text
fn cmd_help(tty: &mut Tty) {
    tty.write_str(
        "BeskarOS Shell - Available commands:\n  \
            clear       - Clear the terminal screen\n  \
            echo [text] - Echo arguments to the console\n  \
            exit        - Exit the shell\n  \
            help        - Display this help text\n  \
            rand [n]    - Generate random bytes\n\
        ",
    );
}

/// Clear the terminal screen
fn cmd_clear(tty: &mut Tty) {
    tty.clear_screen();
}

/// Echo arguments to the console
fn cmd_echo(args: &[String], tty: &mut Tty) {
    if !args.is_empty() {
        let output = args.join(" ");
        tty.write_str(&output);
    }
    tty.write_str("\n");
}

fn cmd_rand(args: &[String], tty: &mut Tty) -> CommandResult {
    const DEFAULT_NUM_BYTES: usize = 16;
    const MAX_NUM_BYTES: usize = 1024;

    let num_bytes = if let Some(x) = args.first() {
        x.parse::<usize>()
            .map_err(|_| "Invalid number of bytes".to_string())?
    } else {
        DEFAULT_NUM_BYTES
    };

    if num_bytes == 0 || num_bytes > MAX_NUM_BYTES {
        return Err(alloc::format!(
            "Number of bytes must be between 1 and {MAX_NUM_BYTES}"
        ));
    }

    let mut buffer = alloc::vec![0u8; num_bytes];
    beskar_lib::rand::rand_fill(&mut buffer)
        .map_err(|e| alloc::format!("Random generation failed: {e:?}"))?;

    tty.write_str("Random Bytes: ");
    let mut str_buf = alloc::string::String::new();
    for byte in &buffer {
        // tty.write_fmt(format_args!("{byte:02X} ")).unwrap();
        str_buf.write_fmt(format_args!("{byte:02X} ")).unwrap();
    }
    tty.write_str(&str_buf);
    tty.write_str("\n");

    Ok(())
}
