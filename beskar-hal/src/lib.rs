//! Core functionality for Beskar OS.
#![no_std]
#![allow(
    clippy::missing_panics_doc,
    clippy::missing_errors_doc,
    clippy::missing_safety_doc,
    clippy::doc_markdown
)]

cfg_select! {
    target_arch = "aarch64" => {
        mod aarch64;
        pub use aarch64::*;
    }
    target_arch = "x86_64" => {
        mod x86_64;
        pub use x86_64::*;
    }
    _ => {
        compile_error!("Unsupported architecture");
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Architecture {
    X86_64,
    AArch64,
}

#[must_use]
#[inline]
pub const fn current_arch() -> Architecture {
    cfg_select! {
        target_arch = "aarch64" => {
            Architecture::AArch64
        }
        target_arch = "x86_64" => {
            Architecture::X86_64
        }
        _ => {
            unimplemented!()
        }
    }
}
