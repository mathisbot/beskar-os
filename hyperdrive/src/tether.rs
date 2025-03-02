//! Smart pointer
//!
//! `Tether` is a structure that allows to obtain an immutable reference to an object,
//! either by knowing a direct reference to it or by having its ownership.
//!
//! `TetherMut` acts the same, except that it allows to obtain a mutable reference.
//!
//! Note that tethers are different from `alloc::borrow::Cow`, because they do not allow to
//! escalade from an immutable reference to a mutable one.
//!
//! ## Example
//!
//! ```rust
//! # use hyperdrive::tether::Tether;
//! #
//! static PRIOR_OBJECT: u32 = 42;
//!
//! let tether = Tether::Reference(&PRIOR_OBJECT);
//! assert_eq!(*tether, 42);
//! let other_tether = Tether::Owned(0);
//! assert_eq!(*other_tether, 0);
//! ```
use core::ops::{Deref, DerefMut};

/// `Tether` is a structure that allows to obtain an immutable reference to an object,
/// either by knowing a direct reference to it or by having its ownership.
pub enum Tether<'a, T> {
    Reference(&'a T),
    Owned(T),
}

impl<T> Deref for Tether<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match self {
            Self::Reference(reference) => reference,
            Self::Owned(owned) => owned,
        }
    }
}

impl<T> Tether<'_, T> {
    #[must_use]
    #[inline]
    /// Take ownership of the object, if it is owned.
    pub fn take(self) -> Option<T> {
        match self {
            Self::Reference(_) => None,
            Self::Owned(owned) => Some(owned),
        }
    }

    #[must_use]
    #[inline]
    /// Returns true if the object is owned.
    pub const fn is_owned(&self) -> bool {
        matches!(self, Self::Owned(_))
    }
}

impl<T: Clone> Tether<'_, T> {
    #[must_use]
    #[inline]
    /// Converts the `Tether` into its owned form, cloning the object if necessary.
    pub fn into_owned(self) -> Self {
        match self {
            Self::Reference(reference) => Self::Owned(reference.clone()),
            Self::Owned(_) => self,
        }
    }
}

/// `TetherMut` is a structure that allows to obtain a mutable reference to an object,
/// either by knowing a direct reference to it or by having its ownership.
pub enum TetherMut<'a, T> {
    Reference(&'a mut T),
    Owned(T),
}

impl<T> Deref for TetherMut<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match self {
            Self::Reference(reference) => reference,
            Self::Owned(owned) => owned,
        }
    }
}

impl<T> DerefMut for TetherMut<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            Self::Reference(reference) => reference,
            Self::Owned(owned) => owned,
        }
    }
}

impl<T> TetherMut<'_, T> {
    #[must_use]
    #[inline]
    /// Take ownership of the object, if it is owned.
    pub fn take(self) -> Option<T> {
        match self {
            Self::Reference(_) => None,
            Self::Owned(owned) => Some(owned),
        }
    }

    #[must_use]
    #[inline]
    /// Returns true if the object is owned.
    pub const fn is_owned(&self) -> bool {
        matches!(self, Self::Owned(_))
    }
}

impl<T: Clone> TetherMut<'_, T> {
    #[must_use]
    #[inline]
    /// Converts the `TetherMut` into its owned form, cloning the object if necessary.
    pub fn into_owned(self) -> Self {
        match self {
            Self::Reference(reference) => Self::Owned(reference.clone()),
            Self::Owned(_) => self,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tether() {
        static PRIOR_OBJECT: u32 = 42;

        let tether = Tether::Reference(&PRIOR_OBJECT);
        assert_eq!(*tether, 42);
        assert!(!tether.is_owned());
        let other_tether = Tether::Owned(0);
        assert_eq!(*other_tether, 0);
        assert!(other_tether.is_owned());
    }

    #[test]
    fn test_tether_mut() {
        let mut prior_object = 42;

        let mut tether = TetherMut::Reference(&mut prior_object);
        assert_eq!(*tether, 42);
        *tether = 0;
        assert_eq!(*tether, 0);
        let mut other_tether = TetherMut::Owned(0);
        assert_eq!(*other_tether, 0);
        *other_tether = 42;
        assert_eq!(*other_tether, 42);
    }
}
