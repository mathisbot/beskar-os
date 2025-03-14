//! Hyperdrive is a utility library for `BeskarOS`.
//!
//! It provides a set of utilities that are necessary to rip through the galaxy BLAZINGLY fast!
//!
//! ## Modules
//!
//! - `locks`: Provides a set of locks that can be used to synchronize access to shared resources.
//! - `once`: Provides a convenient wrapper that can be used to lazily initialize a value.
//! - `queues`: Provides a set of queues that can be used to communicate between threads.
//! - `sync`: Provides a set of synchronization primitives that can be used to synchronize threads.
//! - `tether`: Provides a convenient way to borrow an object, either from a reference or an owned object.
//! - `volatile`: Provides a wrapper for volatile memory accesses and compile-time access rights.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_op_in_unsafe_fn)]
#![warn(clippy::pedantic, clippy::nursery)]

pub mod locks;
pub mod once;
pub mod queues;
pub mod sync;
pub mod tether;
pub mod volatile;
