#![allow(dead_code)]

use core::cell::UnsafeCell;
use core::mem::MaybeUninit;
use core::ops::{Deref, DerefMut};
use core::ptr;
use core::sync::atomic::{AtomicBool, AtomicPtr, Ordering};

/// Mellor-Crummey and Scott lock.
pub struct McsLock<T> {
    tail: AtomicPtr<McsNode>,
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
pub struct McsNode {
    locked: AtomicBool,
    next: UnsafeCell<*mut McsNode>,
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
    pub fn lock<'s, 'node>(&'s self, node: &'node McsNode) -> McsGuard<'node, 's, T> {
        // Make sure node is well initialized
        node.locked.store(true, Ordering::Relaxed);
        unsafe { node.next.get().write(ptr::null_mut()) };

        // Place the node at the end of the queue
        let prev = self
            .tail
            .swap(core::ptr::from_ref(node).cast_mut(), Ordering::AcqRel);

        if !prev.is_null() {
            unsafe {
                (*prev)
                    .next
                    .get()
                    .write(core::ptr::from_ref(node).cast_mut());
            };

            // Wait until the node is at the front of the queue
            while node.locked.load(Ordering::Acquire) {
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
        let node = McsNode::new();
        let mut guard = self.lock(&node);
        f(&mut guard)
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
            next: UnsafeCell::new(ptr::null_mut()),
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
        unsafe {
            // Check if the node is at the front of the queue
            if self.node.next.get().read().is_null() {
                // Atomically set the tail to null
                if self
                    .lock
                    .tail
                    .compare_exchange(
                        core::ptr::from_ref(self.node).cast_mut(),
                        ptr::null_mut(),
                        Ordering::Release,
                        Ordering::Relaxed,
                    )
                    .is_ok()
                {
                    return;
                }
                // If setting the tail to null fails, it means a new node it being added.
                // In such a case, wait until it is completely added (should be fast).
                while self.node.next.get().read().is_null() {
                    core::hint::spin_loop();
                }
            }
            // Unlock the next node
            (*self.node.next.get().read())
                .locked
                .store(false, Ordering::Release);
        }
    }
}

/// Maybe Uninit MCS Lock.
///
/// This is a wrapper around a MCS lock that allows to lock a `MaybeUninit` value.
///
/// ## Example
///
/// ```rust
/// use kernel::utils::locks::{MUMcsLock, McsNode};
///
/// struct MyStruct {}
///
/// static MY_STATIC_STRUCT: MUMcsLock<MyStruct> = McsLock::uninit();
///
/// fn main() {
///     // Initialize the struct
///     MY_STATIC_STRUCT.init(MyStruct {});
///     
///     // Access the struct
///     let node = McsNode::new();
///     let my_struct = MY_STATIC_STRUCT.lock(&node);
///     // ...
/// }
/// ```
pub struct MUMcsLock<T> {
    inner_lock: McsLock<MaybeUninit<T>>,
    is_init: AtomicBool,
}

// `MaybeUninit<T>` doesn't drop `T` when it goes out of scope.
// So we have to manually drop the value when the lock is dropped.
impl<T> Drop for MUMcsLock<T> {
    fn drop(&mut self) {
        if self.is_initialized() {
            // If anyone de-initializes the lock exactly between these two lines,
            // it will result in undefined behavior.
            // This shouldn't happen if the lock is being dropped.
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

    /// Initializes the lock with a value.
    ///
    /// This function can only be called once.
    pub fn init(&self, value: T) {
        let node = McsNode::new();
        let mut inner_self = self.inner_lock.lock(&node);

        let previous_init_value = self.is_init.swap(true, Ordering::AcqRel);
        assert!(!previous_init_value, "MUMcsLock already initialized");
        // Note that, because the inner `McsLock` is locked, it is safe to only check the initialized
        // state once, as no other thread can access the lock until it is unlocked.
        inner_self.write(value);
    }

    /// Returns true if the lock is initialized.
    #[must_use]
    #[inline]
    pub fn is_initialized(&self) -> bool {
        self.is_init.load(Ordering::Acquire)
    }

    /// Locks the lock and returns a guard.
    ///
    /// Panics if the lock is not initialized.
    pub fn lock<'s, 'node>(&'s self, node: &'node McsNode) -> MUMcsGuard<'node, 's, T> {
        // Panicking before locking the inner lock has two effects:
        // - It doesn't poison the lock
        // - If anyone de-initializes the lock exactly after this check,
        //   it will result in undefined behavior.
        //   De-initializing is not a thing right now.
        assert!(self.is_initialized(), "MUMcsLock not initialized");
        let guard = self.inner_lock.lock(node);
        // If de-initialization is implemented, this line should be uncommented.
        // It can cause poisoning in the very specific case mentioned above.
        // assert!(self.is_initialized(), "MUMcsLock not initialized");
        MUMcsGuard { inner_guard: guard }
    }

    /// Try to lock the lock if it has been initialized.
    ///
    /// Returns `None` if the lock has not been initialized.
    pub fn lock_if_init<'s, 'node>(
        &'s self,
        node: &'node McsNode,
    ) -> Option<MUMcsGuard<'node, 's, T>> {
        if self.is_initialized() {
            // If anyone de-initializes the lock exactly between these two lines,
            // there could still be a panic.
            // De-initializing is not a thing right now.
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
        let node = McsNode::new();
        let mut guard = self.lock(&node);
        f(&mut guard)
    }

    #[inline]
    /// Try to lock the lock and call the closure with the guard if possible.
    pub fn try_with_locked<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&mut T) -> R,
    {
        let node = McsNode::new();
        self.lock_if_init(&node).map(|mut guard| f(&mut guard))
    }
}

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
