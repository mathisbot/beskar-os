//! Shell: command parsing, builtin registry, and execution.

mod builtins;
mod dispatch;

pub use dispatch::Shell;
