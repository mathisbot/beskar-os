//! Shell command implementations
use crate::video::tty::Tty;
use alloc::{
    string::{String, ToString},
    vec::Vec,
};

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
            clear - Clear the terminal screen\n  \
            echo  - Echo arguments to the console\n  \
            exit  - Exit the shell\n  \
            help  - Display this help text\n\
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
