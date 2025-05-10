#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_op_in_unsafe_fn)]
#![warn(clippy::pedantic, clippy::nursery)]
#![allow(clippy::missing_errors_doc, clippy::missing_panics_doc)]

extern crate alloc;
pub use beskar_core::storage::{BlockDevice, BlockDeviceError, KernelDevice};

pub mod fs;
pub mod partition;
pub mod vfs;
