#![cfg_attr(not(test), no_std)]
#![allow(clippy::missing_errors_doc, clippy::missing_panics_doc)]

extern crate alloc;
pub use beskar_core::storage::{BlockDevice, BlockDeviceError, KernelDevice};

pub mod fs;
pub mod partition;
pub mod vfs;
