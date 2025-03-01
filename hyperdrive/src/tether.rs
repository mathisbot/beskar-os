//! Tether` is a structure that allows to obtain an immutable reference to an object,
//! either by knowing a direct reference to it or by having its ownership.
//!
//! ## Example
//!
//! ```rust
//! # use hyperdrive::tether::Tether;
//! #
//! static PRIOR_OBJECT: u32 = 42;
//!
//! let tether = Tether::Reference(&PRIOR_OBJECT);
//! assert!(*tether == 42);
//! let other_tether = Tether::Owned(0);
//! assert!(*other_tether == 0);
//! ```

use core::ops::Deref;

/// Tether` is a structure that allows to obtain an immutable reference to an object,
/// either by knowing a direct reference to it or by having its ownership.
pub enum Tether<'a, T> {
    Reference(&'a T),
    Owned(T),
}

impl<T> Deref for Tether<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match self {
            Tether::Reference(reference) => reference,
            Tether::Owned(owned) => owned,
        }
    }
}
