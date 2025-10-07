//! Smart pointer
//!
//! `View` is a structure that allows to obtain an immutable reference to an object,
//! either by knowing a type that implements `Borrow<T>` to it or by having its ownership.
//!
//! `ViewMut` acts the same, except that it allows to obtain a mutable reference.
//!
//! Note that views are different from `alloc::borrow::Cow`, because they do not allow to
//! escalade from an immutable reference to a mutable one.
//!
//! ## `ViewRef` and `ViewMutRef`
//!
//! As the most useful case is to have a reference to an object, `ViewRef` and `ViewMutRef` are
//! type aliases for `View` and `ViewMut` with a reference to the object.
//!
//! ## Example
//!
//! ```rust
//! # use hyperdrive::ptrs::view::ViewRef;
//! #
//! static PRIOR_OBJECT: u8 = 42;
//!
//! let view = ViewRef::<u8>::new_borrow(&PRIOR_OBJECT);
//! assert_eq!(*view, 42);
//! let other_view = ViewRef::<u8>::new_owned(0);
//! assert_eq!(*other_view, 0);
//! ```
//!
use core::borrow::{Borrow, BorrowMut};
use core::ops::{Deref, DerefMut};

/// `View` is a structure that allows to obtain an immutable reference to an object,
/// either by knowing a direct reference to it or by having its ownership.
pub enum View<T, B: Borrow<T>> {
    Borrow(B),
    Owned(T),
}

/// `ViewRef` is a type alias for `View` with a reference to the object.
pub type ViewRef<'a, T> = View<T, &'a T>;

impl<T, B: Borrow<T>> Deref for View<T, B> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match self {
            Self::Borrow(borrow) => borrow.borrow(),
            Self::Owned(owned) => owned,
        }
    }
}

impl<T, B: Borrow<T>> View<T, B> {
    #[must_use]
    #[inline]
    /// Create a new `View` from a reference to an object.
    pub const fn new_borrow(borrow: B) -> Self {
        Self::Borrow(borrow)
    }

    #[must_use]
    #[inline]
    /// Create a new `View` from an owned object.
    pub const fn new_owned(owned: T) -> Self {
        Self::Owned(owned)
    }

    #[must_use]
    #[inline]
    /// Take ownership of the object, if it is owned.
    pub fn take(self) -> Option<T> {
        match self {
            Self::Borrow(_) => None,
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

impl<T: Clone, B: Borrow<T>> View<T, B> {
    #[must_use]
    #[inline]
    /// Converts the `View` into its owned form, cloning the object if necessary.
    pub fn into_owned(self) -> Self {
        match self {
            Self::Borrow(borrow) => Self::Owned(borrow.borrow().clone()),
            Self::Owned(_) => self,
        }
    }
}

impl<T, B: Borrow<T>> Borrow<T> for View<T, B> {
    #[inline]
    fn borrow(&self) -> &T {
        match self {
            Self::Borrow(borrow) => borrow.borrow(),
            Self::Owned(owned) => owned,
        }
    }
}

/// `ViewMut` is a structure that allows to obtain a mutable reference to an object,
/// either by knowing a direct reference to it or by having its ownership.
pub enum ViewMut<T, B: BorrowMut<T>> {
    BorrowMut(B),
    Owned(T),
}

/// `ViewMutRef` is a type alias for `ViewMut` with a reference to the object.
pub type ViewMutRef<'a, T> = ViewMut<T, &'a mut T>;

impl<T, B: BorrowMut<T>> Deref for ViewMut<T, B> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match self {
            Self::BorrowMut(borrow_mut) => borrow_mut.borrow(),
            Self::Owned(owned) => owned,
        }
    }
}

impl<T, B: BorrowMut<T>> DerefMut for ViewMut<T, B> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            Self::BorrowMut(borrow_mut) => borrow_mut.borrow_mut(),
            Self::Owned(owned) => owned,
        }
    }
}

impl<T, B: BorrowMut<T>> ViewMut<T, B> {
    #[must_use]
    #[inline]
    /// Create a new `View` from a reference to an object.
    pub const fn new_borrow(borrow_mut: B) -> Self {
        Self::BorrowMut(borrow_mut)
    }

    #[must_use]
    #[inline]
    /// Create a new `View` from an owned object.
    pub const fn new_owned(owned: T) -> Self {
        Self::Owned(owned)
    }

    #[must_use]
    #[inline]
    /// Take ownership of the object, if it is owned.
    pub fn take(self) -> Option<T> {
        match self {
            Self::BorrowMut(_) => None,
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

impl<T: Clone, B: BorrowMut<T>> ViewMut<T, B> {
    #[must_use]
    #[inline]
    /// Converts the `ViewMut` into its owned form, cloning the object if necessary.
    pub fn into_owned(self) -> Self {
        match self {
            Self::BorrowMut(borrow_mut) => Self::Owned(borrow_mut.borrow().clone()),
            Self::Owned(_) => self,
        }
    }
}

impl<T, B: BorrowMut<T>> Borrow<T> for ViewMut<T, B> {
    #[inline]
    fn borrow(&self) -> &T {
        match self {
            Self::BorrowMut(borrow_mut) => borrow_mut.borrow(),
            Self::Owned(owned) => owned,
        }
    }
}

impl<T, B: BorrowMut<T>> BorrowMut<T> for ViewMut<T, B> {
    #[inline]
    fn borrow_mut(&mut self) -> &mut T {
        match self {
            Self::BorrowMut(borrow_mut) => borrow_mut.borrow_mut(),
            Self::Owned(owned) => owned,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_view() {
        static PRIOR_OBJECT: u8 = 42;

        let view = ViewRef::<u8>::new_borrow(&PRIOR_OBJECT);
        assert_eq!(*view, 42);
        assert!(!view.is_owned());
        let other_view = ViewRef::<u8>::new_owned(0);
        assert_eq!(*other_view, 0);
        assert!(other_view.is_owned());
    }

    #[test]
    fn test_view_mut() {
        let mut prior_object = 42;

        let mut view = ViewMutRef::<u8>::new_borrow(&mut prior_object);
        assert_eq!(*view, 42);
        *view = 0;
        assert_eq!(*view, 0);
        let mut other_view = ViewMutRef::<u8>::new_owned(0);
        assert_eq!(*other_view, 0);
        *other_view = 42;
        assert_eq!(*other_view, 42);
    }

    #[test]
    fn test_view_box() {
        let prior_object = Box::new(42);

        let view = View::<u8, Box<u8>>::new_borrow(prior_object);
        assert_eq!(*view, 42);
        assert!(!view.is_owned());
    }

    #[test]
    fn test_view_take() {
        let view = ViewRef::<u8>::new_borrow(&42);
        assert!(view.take().is_none());
        let other_view = ViewRef::<u8>::new_owned(0);
        assert_eq!(other_view.take(), Some(0));

        let mut x = 42;
        let view = ViewMutRef::<u8>::new_borrow(&mut x);
        assert!(view.take().is_none());
        let other_view = ViewMutRef::<u8>::new_owned(0);
        assert_eq!(other_view.take(), Some(0));
    }

    #[test]
    fn test_view_into_own() {
        let view = ViewRef::<u8>::new_borrow(&42);
        let owned_view = view.into_owned();
        assert!(owned_view.is_owned());
        assert_eq!(*owned_view, 42);
        let other_view = ViewRef::<u8>::new_owned(0);
        let other_owned_view = other_view.into_owned();
        assert!(other_owned_view.is_owned());
        assert_eq!(*other_owned_view, 0);

        let mut x = 42;
        let view = ViewMutRef::<u8>::new_borrow(&mut x);
        let owned_view = view.into_owned();
        assert!(owned_view.is_owned());
        assert_eq!(*owned_view, 42);
        let other_view = ViewMutRef::<u8>::new_owned(0);
        let other_owned_view = other_view.into_owned();
        assert!(other_owned_view.is_owned());
        assert_eq!(*other_owned_view, 0);
    }
}
