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
//! - `ptrs`: Provides a set of smart pointer utilities.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_op_in_unsafe_fn)]
#![warn(clippy::pedantic, clippy::nursery)]

pub mod locks;
pub mod once;
pub mod ptrs;
pub mod queues;
pub mod sync;
