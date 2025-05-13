//! Core functionality for Beskar OS.
#![no_std]
#![forbid(unsafe_op_in_unsafe_fn)]
#![warn(clippy::pedantic, clippy::nursery)]
#![allow(
    clippy::missing_panics_doc,
    clippy::missing_errors_doc,
    clippy::missing_safety_doc,
    clippy::doc_markdown
)]
#![cfg_attr(target_arch = "x86_64", feature(abi_x86_interrupt))]

#[cfg(target_arch = "aarch64")]
mod aarch64;
#[cfg(target_arch = "x86_64")]
mod x86_64;

#[cfg(target_arch = "aarch64")]
pub use aarch64::*;
#[cfg(target_arch = "x86_64")]
pub use x86_64::*;
