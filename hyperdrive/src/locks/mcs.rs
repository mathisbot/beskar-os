//! Mellor-Crummey and Scott lock.
//!
//! This module contains the implementation of the Mellor-Crummey and Scott lock.
//! It is a synchronization primitive that is an evolution of the ticket lock and spinlock.
//!
//! ## Usage
//!
//! The two main structures in this module are `McsLock` and `MUMcsLock`.
//! The second one being a wrapper around the first one that allows to safely lock a `MaybeUninit` value.
//!
//! These structure accept a generic type `T` that is the type of the data protected by the lock.
//! The second generic type `B` is the back-off strategy used by the lock.
//!
//! Note that rustc currently requires that you at least specify either the back-off strategy
//! (and will infer the type of `T`) or the type of `T` (and will use the default `Spin`
//! back-off strategy).
//!
//! ```rust
//! # use hyperdrive::locks::mcs::{McsLock, McsNode};
//! # use hyperdrive::locks::Spin;
//! #
//! let lock = McsLock::<u32>::new(0); // `Spin` is used
//! let lock = McsLock::<_, Spin>::new(0); // `T` is inferred
//! ```
//!
//! ```rust,compile_fail
//! # use hyperdrive::locks::mcs::{MUMcsLock, McsNode};
//! let lock = McsLock::new(0_u32);
//! ```
//!
//! ### `McsLock`
//!
//! To access the content of the lock, use the `with_locked` method.
//! This method is a convenient wrapper around the `lock` method.
//!
//! ```rust
//! # use hyperdrive::locks::mcs::McsLock;
//! #
//! let lock = McsLock::<u8>::new(0);
//!
//! let res = lock.with_locked(|value| {
//!     *value = 42;
//!     *value
//! });
//! assert_eq!(res, 42);
//! ```
//!
//! If you want to avoid blocking locks, you can use the `try_with_locked` method.
//!
//! ```rust
//! # use hyperdrive::locks::mcs::McsLock;
//! #
//! let lock = McsLock::<u8>::new(0);
//!
//! let res = lock.try_with_locked(|value| {
//!     *value = 42;
//!     *value
//! });
//! assert_eq!(res, Some(42));
//! ```
//!
//! If you need a more fine-grained control over the lock, you can use the `lock` method,
//! which lets you handle the guard manually.
//!
//! `McsNode` is a structure used to queue the locks, and they must only be used on one lock
//! at once. Rust's borrow checker won't let you do otherwise anyway.
//!
//! Note that guards will unlock the lock automatically on drop.
//!
//! ```rust
//! # use hyperdrive::locks::mcs::{McsLock, McsNode};
//! #
//! let lock = McsLock::<u8>::new(0);
//! let mut node = McsNode::new();
//!
//! let mut guard = lock.lock(&mut node);
//! *guard = 42;
//! assert_eq!(*guard, 42);
//! drop(guard);
//!
//! // The lock is free again!
//! ```
//!
//! ### `MUMcsLock`
//!
//! `MUMcsLock` are similar to `McsLock`, except they are initialized with a `MaybeUninit` value.
//! This allows lazy-like initialization of the lock.
//!
//! ```rust
//! # use hyperdrive::locks::mcs::{MUMcsLock, McsNode};
//! #
//! static MY_STATIC_STRUCT: MUMcsLock<u16> = MUMcsLock::uninit();
//!
//! MY_STATIC_STRUCT.init(42);
//!
//! let current_value = MY_STATIC_STRUCT.with_locked(|value| {
//!     *value
//! });
//! assert_eq!(current_value, 42);
//! ```
//!
//! There is a new method, `with_locked_if_init`, that allows to try to lock the lock if it has been initialized,
//! returning an `Option` instead of panicking.
//!
//! ```rust
//! # use hyperdrive::locks::mcs::{MUMcsLock, McsNode};
//! #
//! static MY_STATIC_STRUCT: MUMcsLock<u16> = MUMcsLock::uninit();
//!
//! let current_value = MY_STATIC_STRUCT.with_locked_if_init(|value| {
//!     *value
//! });
//! assert!(current_value.is_none());
//! ```

use super::{BackOff, Spin};
use core::cell::UnsafeCell;
use core::marker::PhantomData;
use core::mem::MaybeUninit;
use core::ops::{Deref, DerefMut};
use core::ptr::{self, NonNull};
use core::sync::atomic::{AtomicBool, AtomicPtr, Ordering};

#[derive(Default)]
/// Mellor-Crummey and Scott lock.
pub struct McsLock<T, B: BackOff = Spin> {
    /// Tail of the queue.
    tail: AtomicPtr<McsNode>,
    /// Data protected by the lock.
    data: UnsafeCell<T>,
    /// Back-off strategy.
    _back_off: PhantomData<B>,
}

// Safety:
// Mellor-Crummey and Scott lock is a synchronization primitive.
#[expect(
    clippy::non_send_fields_in_send_ty,
    reason = "Synchronization primitive"
)]
unsafe impl<T, B: BackOff> Send for McsLock<T, B> {}
unsafe impl<T, B: BackOff> Sync for McsLock<T, B> {}

/// Node for MCS lock.
///
/// Nodes are basically used as a queue for the lock.
/// Locking with a node means that the node will patiently wait in the queue.
/// Once the node is at the front of the queue, it can access the lock.
///
/// Unless you specifically want to use a node, you shouldn't need to build one yourself.
/// The `with_locked` function will take care of creating a node for you.
pub struct McsNode {
    /// Whether the node is locked (has access to the locked data).
    locked: AtomicBool,
    /// Next node in the queue.
    next: AtomicPtr<McsNode>,
}

impl McsNode {
    #[must_use]
    #[inline]
    /// Returns true if the node is locked.
    fn is_locked(&self) -> bool {
        self.locked.load(Ordering::Acquire)
    }

    #[must_use]
    #[inline]
    fn next(&self) -> Option<NonNull<Self>> {
        NonNull::new(self.next.load(Ordering::Acquire))
    }

    #[inline]
    fn set_next(&self, next: *mut Self) {
        self.next.store(next, Ordering::Release);
    }
}

impl<T, B: BackOff> McsLock<T, B> {
    #[must_use]
    #[inline]
    /// Creates a new MCS lock.
    pub const fn new(value: T) -> Self {
        Self {
            tail: AtomicPtr::new(ptr::null_mut()),
            data: UnsafeCell::new(value),
            _back_off: PhantomData,
        }
    }

    #[must_use]
    /// Locks the MCS lock and returns a guard.
    ///
    /// For single operations, prefer `with_locked`.
    /// This function allows for a more fine-grained control over the duration of the lock.
    pub fn lock<'s, 'node>(&'s self, node: &'node mut McsNode) -> McsGuard<'node, 's, T, B> {
        // Assert the node is ready to be used
        node.locked.store(true, Ordering::Relaxed);
        node.set_next(ptr::null_mut());

        // Place the node at the end of the queue
        let prev = self.tail.swap(node, Ordering::AcqRel);

        if let Some(prev_ptr) = NonNull::new(prev) {
            unsafe { prev_ptr.as_ref() }.set_next(node);

            // Wait until the node is at the front of the queue
            while node.is_locked() {
                B::back_off();
            }
        }

        McsGuard { lock: self, node }
    }

    #[must_use]
    /// Tries to lock the MCS lock and returns a guard.
    /// If it is already in use, does nothing.
    ///
    /// For single operations, prefer `try_with_locked`.
    /// This function allows for a more fine-grained control over the duration of the lock.
    pub fn try_lock<'s, 'node>(
        &'s self,
        node: &'node mut McsNode,
    ) -> Option<McsGuard<'node, 's, T, B>> {
        // Assert the node is ready to be used
        node.set_next(ptr::null_mut());
        // Note: we do not care about `locked` here as this field will never be accessed

        // Try to place the node at the end of the queue
        self.tail
            .compare_exchange(ptr::null_mut(), node, Ordering::Acquire, Ordering::Relaxed)
            .ok()?;

        Some(McsGuard { lock: self, node })
    }

    #[inline]
    /// Locks the lock and calls the closure with the guard.
    pub fn with_locked<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut T) -> R,
    {
        let mut node = McsNode::new();
        let mut guard = self.lock(&mut node);
        f(&mut guard)
    }

    #[inline]
    /// Locks the lock and calls the closure with the guard.
    pub fn try_with_locked<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&mut T) -> R,
    {
        let mut node = McsNode::new();
        let mut guard = self.try_lock(&mut node)?;
        Some(f(&mut guard))
    }

    #[must_use]
    #[inline]
    #[expect(clippy::mut_from_ref, reason = "Force lock")]
    /// Force access to the data.
    ///
    /// ## Safety
    ///
    /// Caller is responsible for ensuring there are no data races.
    pub unsafe fn force_lock(&self) -> &mut T {
        unsafe { &mut *self.data.get() }
    }

    #[must_use]
    #[inline]
    /// Consume the lock and returns the inner data.
    pub fn into_inner(self) -> T {
        self.data.into_inner()
    }
}

impl Default for McsNode {
    /// Creates a new node for the MCS lock.
    fn default() -> Self {
        Self::new()
    }
}

impl McsNode {
    #[must_use]
    #[inline]
    /// Creates a new node for the MCS lock.
    pub const fn new() -> Self {
        Self {
            locked: AtomicBool::new(false),
            next: AtomicPtr::new(ptr::null_mut()),
        }
    }
}

/// RAII guard for MCS lock.
pub struct McsGuard<'node, 'lock, T, B: BackOff = Spin> {
    lock: &'lock McsLock<T, B>,
    node: &'node McsNode,
}

impl<T, B: BackOff> Deref for McsGuard<'_, '_, T, B> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.lock.data.get() }
    }
}

impl<T, B: BackOff> DerefMut for McsGuard<'_, '_, T, B> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<T, B: BackOff> Drop for McsGuard<'_, '_, T, B> {
    fn drop(&mut self) {
        // Check if the node is the back of the queue
        if self.node.next().is_none() {
            if self
                .lock
                .tail
                .compare_exchange(
                    ptr::from_ref(self.node).cast_mut(),
                    ptr::null_mut(),
                    Ordering::Release,
                    Ordering::Relaxed,
                )
                .is_ok()
            {
                return;
            }

            // If setting the tail to null fails, it means a new node is being added.
            // In such a case, wait until it is completely added.
            // As this operation should be very fast, we can afford to spin here.
            while self.node.next().is_none() {
                core::hint::spin_loop();
            }
        }

        // Unlock the next node
        unsafe { self.node.next().unwrap().as_ref() }
            .locked
            .store(false, Ordering::Release);
    }
}

/// Maybe Uninit MCS Lock.
///
/// This is a wrapper around a MCS lock that allows to lock a `MaybeUninit` value.
pub struct MUMcsLock<T, B: BackOff = Spin> {
    inner_lock: McsLock<MaybeUninit<T>, B>,
    is_init: AtomicBool,
}

// `MaybeUninit<T>` doesn't drop `T` when it goes out of scope.
// So we have to manually drop the value when the lock is dropped.
impl<T, B: BackOff> Drop for MUMcsLock<T, B> {
    fn drop(&mut self) {
        if self.is_initialized() {
            unsafe { self.inner_lock.data.get_mut().assume_init_drop() };
        }
    }
}

impl<T, B: BackOff> Default for MUMcsLock<T, B> {
    fn default() -> Self {
        Self::uninit()
    }
}

impl<T, B: BackOff> MUMcsLock<T, B> {
    #[must_use]
    #[inline]
    /// Creates a new uninitialized `MUMcsLock`.
    pub const fn uninit() -> Self {
        Self {
            inner_lock: McsLock::new(MaybeUninit::uninit()),
            is_init: AtomicBool::new(false),
        }
    }

    /// Returns true if the lock is initialized.
    #[must_use]
    #[inline]
    pub fn is_initialized(&self) -> bool {
        self.is_init.load(Ordering::Acquire)
    }

    /// Initializes the lock with a value.
    ///
    /// If the lock is already initialized, this function doesn't change the value.
    pub fn init(&self, value: T) {
        // Skip locking if the lock is already initialized
        if self.is_initialized() {
            return;
        }

        self.inner_lock.with_locked(|inner| {
            // Check a second time in case the lock was initialized between the two lines
            if !self.is_init.swap(true, Ordering::AcqRel) {
                inner.write(value);
            }
        });
    }

    #[must_use]
    #[inline]
    /// Locks the lock and returns a guard.
    ///
    /// ## Panics
    ///
    /// Panics if the lock is not initialized.
    pub fn lock<'s, 'node>(&'s self, node: &'node mut McsNode) -> MUMcsGuard<'node, 's, T, B> {
        // Panicking before locking the inner lock so that it doesn't poison the lock
        assert!(self.is_initialized(), "MUMcsLock not initialized");

        let guard = self.inner_lock.lock(node);

        MUMcsGuard { inner_guard: guard }
    }

    #[must_use]
    /// Tries to lock the lock and returns a guard.
    /// If it is already in use or isn't initialized, does nothing.
    ///
    /// If you need a function that only abort if the lock is not initialized, use `lock_if_init`.
    pub fn try_lock<'s, 'node>(
        &'s self,
        node: &'node mut McsNode,
    ) -> Option<MUMcsGuard<'node, 's, T, B>> {
        if !self.is_initialized() {
            return None;
        }

        let guard = self.inner_lock.try_lock(node)?;

        Some(MUMcsGuard { inner_guard: guard })
    }

    #[must_use]
    #[inline]
    /// Try to lock the lock if it has been initialized.
    /// Returns `None` if the lock has not been initialized.
    ///
    /// If you need a function that also aborts if the lock is in use, use `try_lock`.
    pub fn lock_if_init<'s, 'node>(
        &'s self,
        node: &'node mut McsNode,
    ) -> Option<MUMcsGuard<'node, 's, T, B>> {
        self.is_initialized().then(move || self.lock(node))
    }

    #[inline]
    /// Locks the lock and calls the closure with the guard.
    ///
    /// Panics if the lock is not initialized.
    pub fn with_locked<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut T) -> R,
    {
        let mut node = McsNode::new();
        let mut guard = self.lock(&mut node);
        f(&mut guard)
    }

    #[inline]
    /// Try to lock the lock and call the closure with the guard if the lock
    /// is initialized.
    pub fn with_locked_if_init<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&mut T) -> R,
    {
        let mut node = McsNode::new();
        self.lock_if_init(&mut node).map(|mut guard| f(&mut guard))
    }

    #[inline]
    /// Try to lock the lock and call the closure with the guard if the lock
    /// is initialized.
    pub fn try_with_locked<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&mut T) -> R,
    {
        let mut node = McsNode::new();
        self.try_lock(&mut node).map(|mut guard| f(&mut guard))
    }

    #[must_use]
    #[inline]
    #[expect(clippy::mut_from_ref, reason = "Force lock")]
    /// Force access to the data.
    ///
    /// ## Safety
    ///
    /// Inner lock must be initialized.
    /// Caller is responsible for ensuring there are no data races.
    pub unsafe fn force_lock(&self) -> &mut T {
        unsafe { self.inner_lock.force_lock().assume_init_mut() }
    }

    #[must_use]
    #[inline]
    /// Consume the lock and returns the inner data if it is initialized.
    pub fn into_inner(mut self) -> Option<T> {
        if self.is_init.swap(false, Ordering::Acquire) {
            // Safety: The lock is initialized.
            // We cannot use `assume_init` as `inner_lock` is part of `self`.
            // Therefore, we need to `assume_init_read` the value, and "uninitialize"
            // the lock to avoid dropping the returned value.
            Some(unsafe { self.inner_lock.data.get_mut().assume_init_read() })
        } else {
            None
        }
    }
}

/// RAII guard for `MUMcsLock` lock.
pub struct MUMcsGuard<'node, 'lock, T, B: BackOff = Spin> {
    inner_guard: McsGuard<'node, 'lock, MaybeUninit<T>, B>,
}

impl<T, B: BackOff> Deref for MUMcsGuard<'_, '_, T, B> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        // Safety: The lock is initialized if the guard exists.
        unsafe { self.inner_guard.assume_init_ref() }
    }
}

impl<T, B: BackOff> DerefMut for MUMcsGuard<'_, '_, T, B> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // Safety: The lock is initialized if the guard exists.
        unsafe { self.inner_guard.assume_init_mut() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Barrier};
    use std::thread::spawn;

    type TestMcsLock<T> = McsLock<T, Spin>;
    type TestMUMcsLock<T> = MUMcsLock<T, Spin>;

    #[test]
    fn test_mcs_lock() {
        let lock = TestMcsLock::new(0);
        let mut node = McsNode::new();

        let mut guard = lock.lock(&mut node);
        *guard = 42;
        assert_eq!(*guard, 42);
    }

    #[test]
    fn test_mcs_try_lock() {
        let lock = TestMcsLock::new(0);
        let mut node = McsNode::new();
        let mut failed_node = McsNode::new();

        let guard = lock.try_lock(&mut node);
        let failed_guard = lock.try_lock(&mut failed_node);
        assert!(guard.is_some());
        assert!(failed_guard.is_none());
        drop(guard);
    }

    #[test]
    fn test_mcs_lock_with_locked() {
        let lock = TestMcsLock::new(0);

        let res = lock.with_locked(|value| {
            *value = 42;
            *value
        });
        assert_eq!(res, 42);
    }

    #[test]
    fn test_mcs_lock_try_with_locked() {
        let lock = TestMcsLock::new(0);
        let mut node = McsNode::new();

        let guard = lock.lock(&mut node);
        let failed_guard = lock.try_with_locked(|value| {
            *value = 42;
            *value
        });
        assert!(failed_guard.is_none());
        drop(guard);
    }

    #[test]
    fn test_mcs_force_lock() {
        let lock = TestMcsLock::new(42);

        let mut node = McsNode::new();
        let guard = lock.lock(&mut node);

        let value = unsafe { *lock.force_lock() };

        assert_eq!(*guard, value);
    }

    #[test]
    fn test_mumcs_lock() {
        let lock = TestMUMcsLock::uninit();
        let mut node = McsNode::new();

        lock.init(42);

        let mut guard = lock.lock(&mut node);
        *guard = 0;
        assert_eq!(*guard, 0);
    }

    #[test]
    #[should_panic = "MUMcsLock not initialized"]
    fn test_mumcs_lock_uninit() {
        let lock = TestMUMcsLock::<u8>::uninit();
        let mut node = McsNode::new();

        let _guard = lock.lock(&mut node);
    }

    #[test]
    fn test_mumcs_try_lock() {
        let lock = TestMUMcsLock::uninit();
        let mut node = McsNode::new();
        let mut failed_node = McsNode::new();

        let failed_guard = lock.try_lock(&mut failed_node);
        assert!(failed_guard.is_none());
        drop(failed_guard);

        lock.init(42);

        let guard = lock.try_lock(&mut node);
        let failed_guard = lock.try_lock(&mut failed_node);
        assert!(guard.is_some());
        assert!(failed_guard.is_none());
        drop(guard);
    }

    #[test]
    fn test_mumcs_lock_if_init() {
        let lock = TestMUMcsLock::uninit();
        let mut node = McsNode::new();
        let mut failed_node = McsNode::new();

        let failed_guard = lock.lock_if_init(&mut failed_node);
        assert!(failed_guard.is_none());
        drop(failed_guard);

        lock.init(42);

        let guard = lock.lock_if_init(&mut node);
        assert!(guard.is_some());
        drop(guard);
    }

    #[test]
    fn test_mumcs_lock_with_locked() {
        let lock = TestMUMcsLock::uninit();

        lock.init(42);

        let res = lock.with_locked(|value| {
            *value = 0;
            *value
        });
        assert_eq!(res, 0);
    }

    #[test]
    fn test_mumcs_lock_with_locked_if_init() {
        let lock = TestMUMcsLock::uninit();

        let res = lock.with_locked_if_init(|value| {
            *value = 0;
            *value
        });
        assert!(res.is_none());

        lock.init(42);

        let res = lock.with_locked_if_init(|value| {
            *value = 0;
            *value
        });
        assert_eq!(res, Some(0));
    }

    #[test]
    fn test_mumcs_lock_try_with_locked() {
        let lock = TestMUMcsLock::uninit();

        let res = lock.try_with_locked(|value| {
            *value = 0;
            *value
        });
        assert!(res.is_none());

        lock.init(42);

        let mut node = McsNode::new();
        let guard = lock.try_lock(&mut node);
        let res = lock.try_with_locked(|value| {
            *value = 0;
            *value
        });
        assert!(guard.is_some());
        assert!(res.is_none());
    }

    #[test]
    #[cfg(miri)]
    fn test_mumcs_drop() {
        let lock = TestMUMcsLock::uninit();
        lock.init(Box::new(42));
        let _uninit = MUMcsLock::<Box<[u64]>>::uninit();
    }

    #[test]
    #[cfg(miri)]
    fn test_mumcs_into_inner() {
        let lock = TestMUMcsLock::uninit();
        lock.init(Box::new(42));
        let _boxed = lock.into_inner().unwrap();
    }

    #[test]
    fn test_concurent() {
        let lock = Arc::new(TestMcsLock::new(0));

        let num_threads = 10;
        let iterations = 50;

        let mut handles = Vec::with_capacity(num_threads);

        for _ in 0..num_threads {
            let handle = spawn({
                let lock = lock.clone();
                move || {
                    for _ in 0..iterations {
                        lock.with_locked(|value| {
                            *value += 1;
                        });
                    }
                }
            });

            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(lock.with_locked(|value| *value), num_threads * iterations);
    }

    #[test]
    fn test_concurent2() {
        let lock = Arc::new(TestMcsLock::new(0));
        let barrier = Arc::new(Barrier::new(2));

        let w = spawn({
            let lock = lock.clone();
            let barrier = barrier.clone();
            move || {
                lock.with_locked(|value| {
                    barrier.wait();
                    for i in 0..=100 {
                        *value = i;
                    }
                });
            }
        });

        let r = spawn({
            let lock = lock.clone();
            let barrier = barrier.clone();
            move || {
                barrier.wait();
                let v = lock.with_locked(|value| *value);
                assert_eq!(v, 100);
            }
        });

        w.join().unwrap();
        r.join().unwrap();
    }
}
