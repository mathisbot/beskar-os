//! Locks related utilities.
//!
//! This module contains the implementation of the locks used
//! to synchronize access to shared resources.
//!
//! ## Modules
//!
//! - `mcs` : Provides an implementation of the MCS lock.
//! - `rw` : Provides an implementation of the read-write lock.

pub mod mcs;
pub mod rw;
