//! Hyperdrive is a utility library for `BeskarOS`.
//!
//! It provides a set of utilities that are necessary to rip through the galaxy BLAZINGLY fast!
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_op_in_unsafe_fn)]
#![warn(clippy::pedantic, clippy::nursery)]

pub mod locks;
pub mod once;
pub mod queues;
pub mod sync;
pub mod volatile;
