//! Locks related utilities.
//!
//! This module contains the implementation of the locks used
//! to synchronize access to shared resources.
//!
//! ## Modules
//!
//! - `mcs` : Provides an implementation of the MCS lock.
//! - `rw` : Provides an implementation of the read-write lock.
//! - `ticket` : Provides an implementation of the ticket lock.
//!
//! ## Back-off Strategy
//!
//! This modules uses a trait, `BackOff`, to define a back-off strategy
//! for locks. This module provides a default implementation of the back-off
//! strategy, which is a spin-wait loop.
//!
//! This trait only has one method, `back_off`, which is called when a thread
//! is unable to acquire a lock.

pub mod mcs;
pub mod rw;
pub mod ticket;

/// A trait that defines a back-off strategy for locks.
///
/// This trait is used to define how a thread should behave when it
/// is unable to acquire a lock. The default implementation is
/// a spin-wait loop, but other strategies can be implemented
/// to yield the CPU or sleep for a certain duration.
pub trait BackOff {
    /// Performs the back-off operation.
    fn back_off();
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
/// A back-off strategy that uses a spin-wait loop.
///
/// To optimize performance and CPU consumption,
/// the function `core::hint::spin_loop` is called.
pub struct Spin;

impl BackOff for Spin {
    #[inline]
    fn back_off() {
        core::hint::spin_loop();
    }
}
