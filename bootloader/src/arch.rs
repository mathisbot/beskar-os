//! Architecture-specific code.

mod aarch64;
mod x86_64;

#[cfg(target_arch = "aarch64")]
pub use aarch64::*;
#[cfg(target_arch = "x86_64")]
pub use x86_64::*;
