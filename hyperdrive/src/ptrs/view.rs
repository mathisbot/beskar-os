//! Smart pointer
//!
//! `View` is a structure that allows to obtain an immutable reference to an object,
//! either by knowing a direct reference to it or by having its ownership.
//!
//! `ViewMut` acts the same, except that it allows to obtain a mutable reference.
//!
//! Note that views are different from `alloc::borrow::Cow`, because they do not allow to
//! escalade from an immutable reference to a mutable one.
//!
//! ## Example
//!
//! ```rust
//! # use hyperdrive::ptrs::view::View;
//! #
//! static PRIOR_OBJECT: u32 = 42;
//!
//! let view = View::Reference(&PRIOR_OBJECT);
//! assert_eq!(*view, 42);
//! let other_view = View::Owned(0);
//! assert_eq!(*other_view, 0);
//! ```
use core::ops::{Deref, DerefMut};

/// `View` is a structure that allows to obtain an immutable reference to an object,
/// either by knowing a direct reference to it or by having its ownership.
pub enum View<'a, T> {
    Reference(&'a T),
    Owned(T),
}

impl<T> Deref for View<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match self {
            Self::Reference(reference) => reference,
            Self::Owned(owned) => owned,
        }
    }
}

impl<T> View<'_, T> {
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

impl<T: Clone> View<'_, T> {
    #[must_use]
    #[inline]
    /// Converts the `View` into its owned form, cloning the object if necessary.
    pub fn into_owned(self) -> Self {
        match self {
            Self::Reference(reference) => Self::Owned(reference.clone()),
            Self::Owned(_) => self,
        }
    }
}

/// `ViewMut` is a structure that allows to obtain a mutable reference to an object,
/// either by knowing a direct reference to it or by having its ownership.
pub enum ViewMut<'a, T> {
    Reference(&'a mut T),
    Owned(T),
}

impl<T> Deref for ViewMut<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match self {
            Self::Reference(reference) => reference,
            Self::Owned(owned) => owned,
        }
    }
}

impl<T> DerefMut for ViewMut<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            Self::Reference(reference) => reference,
            Self::Owned(owned) => owned,
        }
    }
}

impl<T> ViewMut<'_, T> {
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

impl<T: Clone> ViewMut<'_, T> {
    #[must_use]
    #[inline]
    /// Converts the `ViewMut` into its owned form, cloning the object if necessary.
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
    fn test_view() {
        static PRIOR_OBJECT: u32 = 42;

        let view = View::Reference(&PRIOR_OBJECT);
        assert_eq!(*view, 42);
        assert!(!view.is_owned());
        let other_view = View::Owned(0);
        assert_eq!(*other_view, 0);
        assert!(other_view.is_owned());
    }

    #[test]
    fn test_view_mut() {
        let mut prior_object = 42;

        let mut view = ViewMut::Reference(&mut prior_object);
        assert_eq!(*view, 42);
        *view = 0;
        assert_eq!(*view, 0);
        let mut other_view = ViewMut::Owned(0);
        assert_eq!(*other_view, 0);
        *other_view = 42;
        assert_eq!(*other_view, 42);
    }
}
