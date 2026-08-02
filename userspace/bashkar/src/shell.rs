//! Shell: command parsing, builtin registry, and execution.

mod builtins;
mod dispatch;

pub use dispatch::Shell;

pub(super) fn init() {
    builtins::init_start_time();
}
