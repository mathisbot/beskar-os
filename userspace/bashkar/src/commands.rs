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
pub fn execute_command(command: &str, args: &[&str], tty: &mut Tty) -> CommandResult {
    match command {
        "" => CommandResult::Ok(()),
        "help" => {
            cmd_help(tty);
            Ok(())
        }
        "echo" => {
            cmd_echo(args, tty);
            Ok(())
        }
        "exit" => beskar_lib::exit(beskar_lib::ExitCode::Success),
        _ => unknown(command, tty),
    }
}

/// Parse a command line into a command and arguments
pub fn parse_command_line(line: &str) -> (String, Vec<String>) {
    let mut parts: Vec<String> = line.split_whitespace().map(ToString::to_string).collect();

    if parts.is_empty() {
        return (String::new(), Vec::new());
    }

    let command = parts.remove(0);
    (command, parts)
}

/// Display help text
fn cmd_help(tty: &mut Tty) {
    tty.write_str(
        "BeskarOS Shell - Available commands:\n  \
            help  - Display this help text\n  \
            echo  - Echo arguments to the console\n  \
            exit  - Exit the shell\n\
        ",
    );
}

/// Echo arguments to the console
fn cmd_echo(args: &[&str], tty: &mut Tty) {
    let output = args.join(" ");
    tty.write_str(&output);
    tty.write_str("\n");
}

fn unknown(command: &str, tty: &mut Tty) -> CommandResult {
    let string = alloc::format!("Unknown command: {command}\n");
    tty.write_str(&string);
    CommandResult::Err(string)
}
