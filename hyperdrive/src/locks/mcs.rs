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
//! To access the content of the lock, use the `with_locked` method.
//! This method is a convenient wrapper around the `lock` method.
//!
//! ### `McsLock`
//!
//! ```rust
//! # use hyperdrive::locks::mcs::McsLock;
//! #
//! let lock = McsLock::new(0);
//!
//! let res = lock.with_locked(|value| {
//!     *value = 42;
//!     *value
//! });
//! assert_eq!(res, 42);
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
//! let lock = McsLock::new(0);
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
//! There is a new method, `try_with_locked`, that allows to try to lock the lock if it has been initialized,
//! returning an `Option` instead of panicking.
//!
//! ```rust
//! # use hyperdrive::locks::mcs::{MUMcsLock, McsNode};
//! #
//! static MY_STATIC_STRUCT: MUMcsLock<u16> = MUMcsLock::uninit();
//!
//! let current_value = MY_STATIC_STRUCT.try_with_locked(|value| {
//!     *value
//! });
//! assert!(current_value.is_none());
//! ```

use core::cell::UnsafeCell;
use core::mem::MaybeUninit;
use core::ops::{Deref, DerefMut};
use core::ptr::{self, NonNull};
use core::sync::atomic::{AtomicBool, AtomicPtr, Ordering};

/// Mellor-Crummey and Scott lock.
pub struct McsLock<T> {
    /// Tail of the queue.
    tail: AtomicPtr<McsNode>,
    /// Data protected by the lock.
    data: UnsafeCell<T>,
}

// Safety:
// Mellor-Crummey and Scott lock is a synchronization primitive.
#[allow(clippy::non_send_fields_in_send_ty)]
unsafe impl<T> Send for McsLock<T> {}
unsafe impl<T> Sync for McsLock<T> {}

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

impl<T> McsLock<T> {
    #[must_use]
    #[inline]
    /// Creates a new MCS lock.
    pub const fn new(value: T) -> Self {
        Self {
            tail: AtomicPtr::new(ptr::null_mut()),
            data: UnsafeCell::new(value),
        }
    }

    #[must_use]
    /// Locks the MCS lock and returns a guard.
    ///
    /// For single operations, prefer `with_locked`.
    /// This function allows for a more fine-grained control over the duration of the lock.
    pub fn lock<'s, 'node>(&'s self, node: &'node mut McsNode) -> McsGuard<'node, 's, T> {
        // Assert the node is ready to be used
        node.locked.store(true, Ordering::Release);
        node.set_next(ptr::null_mut());

        // Place the node at the end of the queue
        let prev = self.tail.swap(node, Ordering::AcqRel);

        if let Some(prev_ptr) = NonNull::new(prev) {
            unsafe { prev_ptr.as_ref() }.set_next(node);

            // Wait until the node is at the front of the queue
            while node.is_locked() {
                core::hint::spin_loop();
            }
        }

        McsGuard { lock: self, node }
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

    #[must_use]
    #[inline]
    #[allow(clippy::mut_from_ref)]
    /// Force access to the data.
    ///
    /// ## Safety
    ///
    /// Caller is responsible for ensuring there are no data races.
    pub unsafe fn force_lock(&self) -> &mut T {
        unsafe { &mut *self.data.get() }
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
pub struct McsGuard<'node, 'lock, T> {
    lock: &'lock McsLock<T>,
    node: &'node McsNode,
}

impl<T> Deref for McsGuard<'_, '_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.lock.data.get() }
    }
}

impl<T> DerefMut for McsGuard<'_, '_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<T> Drop for McsGuard<'_, '_, T> {
    fn drop(&mut self) {
        // Check if the node is at the front of the queue
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
            // In such a case, wait until it is completely added (should be fast).
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
pub struct MUMcsLock<T> {
    inner_lock: McsLock<MaybeUninit<T>>,
    is_init: AtomicBool,
}

// `MaybeUninit<T>` doesn't drop `T` when it goes out of scope.
// So we have to manually drop the value when the lock is dropped.
impl<T> Drop for MUMcsLock<T> {
    fn drop(&mut self) {
        if self.is_initialized() {
            unsafe { self.inner_lock.data.get_mut().assume_init_drop() };
        }
    }
}

impl<T> Default for MUMcsLock<T> {
    fn default() -> Self {
        Self::uninit()
    }
}

impl<T> MUMcsLock<T> {
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
    /// Locks the lock and returns a guard.
    ///
    /// ## Panics
    ///
    /// Panics if the lock is not initialized.
    pub fn lock<'s, 'node>(&'s self, node: &'node mut McsNode) -> MUMcsGuard<'node, 's, T> {
        // Panicking before locking the inner lock has two effects:
        // - It doesn't poison the lock
        // - If anyone de-initializes the lock exactly after this check,
        //   it will result in undefined behavior.
        assert!(self.is_initialized(), "MUMcsLock not initialized");

        let guard = self.inner_lock.lock(node);

        // If de-initialization is implemented, this line should be uncommented.
        // It can cause poisoning in the very specific case mentioned above.
        // assert!(self.is_initialized(), "MUMcsLock not initialized");

        MUMcsGuard { inner_guard: guard }
    }

    #[must_use]
    /// Try to lock the lock if it has been initialized.
    ///
    /// Returns `None` if the lock has not been initialized.
    pub fn lock_if_init<'s, 'node>(
        &'s self,
        node: &'node mut McsNode,
    ) -> Option<MUMcsGuard<'node, 's, T>> {
        if self.is_initialized() {
            // If anyone de-initializes the lock exactly between these two lines,
            // there could still be a panic.
            Some(self.lock(node))
        } else {
            None
        }
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
    pub fn try_with_locked<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&mut T) -> R,
    {
        let mut node = McsNode::new();
        self.lock_if_init(&mut node).map(|mut guard| f(&mut guard))
    }

    #[must_use]
    #[inline]
    #[allow(clippy::mut_from_ref)]
    /// Force access to the data.
    ///
    /// ## Safety
    ///
    /// Inner lock must be initialized.
    /// Caller is responsible for ensuring there are no data races.
    pub unsafe fn force_lock(&self) -> &mut T {
        unsafe { self.inner_lock.force_lock().assume_init_mut() }
    }
}

/// RAII guard for `MUMcsLock` lock.
pub struct MUMcsGuard<'node, 'lock, T> {
    inner_guard: McsGuard<'node, 'lock, MaybeUninit<T>>,
}

impl<T> Deref for MUMcsGuard<'_, '_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        // Safety: The lock is initialized if the guard exists.
        unsafe { self.inner_guard.assume_init_ref() }
    }
}

impl<T> DerefMut for MUMcsGuard<'_, '_, T> {
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

    #[test]
    fn test_mcs_lock() {
        let lock = McsLock::new(0);
        let mut node = McsNode::new();

        let mut guard = lock.lock(&mut node);
        *guard = 42;
        assert_eq!(*guard, 42);
    }

    #[test]
    fn test_mcs_lock_with_locked() {
        let lock = McsLock::new(0);

        let res = lock.with_locked(|value| {
            *value = 42;
            *value
        });
        assert_eq!(res, 42);
    }

    #[test]
    fn test_mcs_force_lock() {
        let lock = McsLock::new(42);

        let mut node = McsNode::new();
        let guard = lock.lock(&mut node);

        let value = unsafe { *lock.force_lock() };

        assert_eq!(*guard, value);
    }

    #[test]
    fn test_mumcs_lock() {
        let lock = MUMcsLock::uninit();
        let mut node = McsNode::new();

        lock.init(42);

        let mut guard = lock.lock(&mut node);
        *guard = 0;
        assert_eq!(*guard, 0);
    }

    #[test]
    #[should_panic = "MUMcsLock not initialized"]
    fn test_mumcs_lock_uninit() {
        let lock = MUMcsLock::<u8>::uninit();
        let mut node = McsNode::new();

        let _guard = lock.lock(&mut node);
    }

    #[test]
    fn test_mumcs_lock_with_locked() {
        let lock = MUMcsLock::uninit();

        lock.init(42);

        let res = lock.with_locked(|value| {
            *value = 0;
            *value
        });
        assert_eq!(res, 0);
    }

    #[test]
    fn test_mumcs_lock_try_with_locked() {
        let lock = MUMcsLock::uninit();

        let res = lock.try_with_locked(|value| {
            *value = 0;
            *value
        });
        assert!(res.is_none());

        lock.init(42);

        let res = lock.try_with_locked(|value| {
            *value = 0;
            *value
        });
        assert_eq!(res, Some(0));
    }

    #[cfg(miri)]
    #[test]
    fn test_mumcs_drop() {
        let lock = MUMcsLock::uninit();
        lock.init(Box::new(42));
        let _uninit = MUMcsLock::<Box<[u64]>>::uninit();
    }

    #[test]
    fn test_concurent() {
        let lock = Arc::new(McsLock::new(0));

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
        let lock = Arc::new(McsLock::new(0));
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
