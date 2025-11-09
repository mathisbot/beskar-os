//! Prelude module for beskar-lib.
//!
//! This module re-exports commonly used types and traits from beskar-lib
//! to simplify imports in user code.
//!
//! # Examples
//!
//! ```rust
//! use beskar_lib::prelude::*;
//! ```
pub use crate::error::{
    FileError, FileErrorKind, FileResult, IoError, IoErrorKind, IoResult, MemoryError,
    MemoryErrorKind, MemoryResult, SyscallError, SyscallResult,
};
pub use crate::io::{File, Read as _, Seek as _, Write as _};
