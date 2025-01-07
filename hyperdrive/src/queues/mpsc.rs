//! A non-intrusive, multiple-producer single-consumer queue.
//!
//! In order to be used, the element type must implement the `Queueable` trait.
//!
//! ## Example
//!
//! ```rust
//! # use hyperdrive::queues::mpsc::{Link, MpscQueue, Queueable};
//! # use core::pin::Pin;
//! # use core::ptr::NonNull;
//! # use core::mem::offset_of;
//! #
//! # extern crate alloc;
//! # use alloc::boxed::Box;
//! #
//! struct Element {
//!     value: u8,
//!     next: Option<NonNull<Element>>,
//! }
//!
//! impl Unpin for Element {}
//!
//! impl Queueable for Element {
//!     type Handle = Pin<Box<Self>>;
//!
//!     fn release(r: Self::Handle) -> NonNull<Self> {
//!         let ptr = Box::into_raw(Pin::into_inner(r));
//!         unsafe { NonNull::new_unchecked(ptr) }
//!     }
//!     
//!     unsafe fn capture(ptr: NonNull<Self>) -> Self::Handle {
//!         Pin::new(unsafe { Box::from_raw(ptr.as_ptr()) })
//!     }
//!
//!     unsafe fn get_link(ptr: NonNull<Self>) -> NonNull<Link<Self>> {
//!         let base = ptr.as_ptr().cast::<Link<Self>>();
//!         let ptr = unsafe { base.byte_add(offset_of!(Element, next)) };
//!         unsafe { NonNull::new_unchecked(ptr) }
//!     }
//! }
//!
//! let queue: MpscQueue<Element> = MpscQueue::new(Box::pin(Element { value: 0, next: None }));
//! queue.enqueue(Box::pin(Element { value: 1, next: None }));
//! let element = queue.dequeue().unwrap();
//! assert_eq!(element.value, 1);
//! ```
use core::{
    cell::UnsafeCell,
    ptr::{self, NonNull},
    sync::atomic::{AtomicBool, AtomicPtr, Ordering},
};

pub trait Queueable: Sized {
    /// The type of the handle to the link.
    ///
    /// Usually, it is `Pin<Box<Self>>`.
    type Handle;

    /// Takes ownership of the handle and returns a pointer to it.
    fn release(r: Self::Handle) -> NonNull<Self>;

    /// Capture the data pointed to by the pointer and return a handle to it.
    ///
    /// ## Safety
    ///
    /// `ptr` must be a valid pointer to a `Self` instance.
    unsafe fn capture(ptr: NonNull<Self>) -> Self::Handle;

    /// Returns a pointer to the link to the next element.
    ///
    /// Because an `MpscQueue` is non-intrusive, the link has to be provided
    /// already allocated in memory, as a pointer.
    ///
    /// ## Safety
    ///
    /// `ptr` must be a valid pointer to a `Self` instance.
    unsafe fn get_link(ptr: NonNull<Self>) -> NonNull<Link<Self>>;
}

/// Describes a link between two elements in the queue.
pub struct Link<T> {
    /// The next element in the queue.
    next: AtomicPtr<T>,
    /// A phantom field to pin the link.
    _unpin: core::marker::PhantomPinned,
}

impl<T> Default for Link<T> {
    fn default() -> Self {
        Self {
            next: AtomicPtr::new(ptr::null_mut()),
            _unpin: core::marker::PhantomPinned,
        }
    }
}

/// A multiple-producer single-consumer queue.
pub struct MpscQueue<T: Queueable> {
    /// The head of the queue.
    head: AtomicPtr<T>,
    /// The tail of the queue.
    tail: UnsafeCell<*mut T>,
    /// Whether the queue is being dequeued or not.
    being_dequeued: AtomicBool,
    /// The stub node.
    stub: NonNull<T>,
}

// Safety:
// The queue is thread-safe.
#[allow(clippy::non_send_fields_in_send_ty)]
unsafe impl<T: Queueable> Send for MpscQueue<T> {}
unsafe impl<T: Queueable> Sync for MpscQueue<T> {}

/// The result of a dequeue operation.
pub enum DequeueResult<T: Queueable> {
    /// Dequeueing was successful.
    Element(Option<T::Handle>),
    /// The queue is temporarily unavailable,
    /// and the operation should be retried.
    Retry,
    /// The queue is busy.
    InUse,
}

impl<T: Queueable> DequeueResult<T> {
    #[must_use]
    /// Unwraps the result, panicking if it is a `Retry` or `Busy`.
    ///
    /// ## Panics
    ///
    /// Panics if the result is a `Retry` or `Busy`.
    pub fn unwrap(self) -> Option<T::Handle> {
        match self {
            Self::Element(res) => res,
            Self::Retry => panic!("Unwrapped a DequeueResult::Retry"),
            Self::InUse => panic!("Unwrapped a DequeueResult::Busy"),
        }
    }

    #[must_use]
    /// Unwraps the result without checking its value.
    ///
    /// ## Safety
    ///
    /// The caller must ensure that the result is not a `Retry` or `Busy`.
    /// Otherwise, this is immediately undefined behavior.
    ///
    /// Refer to `core::hint::unreachable_unchecked`.
    pub unsafe fn unwrap_unchecked(self) -> Option<T::Handle> {
        let Self::Element(res) = self else {
            unsafe { core::hint::unreachable_unchecked() };
        };

        res
    }
}

impl<T: Queueable> Default for MpscQueue<T>
where
    T::Handle: Default,
{
    fn default() -> Self {
        Self::new(T::Handle::default())
    }
}

impl<T: Queueable> MpscQueue<T> {
    #[must_use]
    pub fn new(stub: T::Handle) -> Self {
        let stub_ptr = <T as Queueable>::release(stub);
        Self {
            head: AtomicPtr::new(stub_ptr.as_ptr()),
            tail: UnsafeCell::new(stub_ptr.as_ptr()),
            being_dequeued: AtomicBool::new(false),
            stub: stub_ptr,
        }
    }

    #[inline]
    pub fn enqueue(&self, element: T::Handle) {
        unsafe {
            self.enqueue_ptr(T::release(element));
        }
    }

    /// ## Safety
    ///
    /// `ptr` must be a valid pointer to a `T` instance.
    unsafe fn enqueue_ptr(&self, ptr: NonNull<T>) {
        unsafe { T::get_link(ptr).as_ref() }
            .next
            .store(ptr::null_mut(), Ordering::Relaxed);

        let prev = self.head.swap(ptr.as_ptr(), Ordering::AcqRel);

        unsafe { T::get_link(NonNull::new_unchecked(prev)).as_ref() }
            .next
            .store(ptr.as_ptr(), Ordering::Release);
    }

    pub fn dequeue(&self) -> Option<T::Handle> {
        let mut state = self.try_dequeue();
        while matches!(state, DequeueResult::Retry | DequeueResult::InUse) {
            core::hint::spin_loop();
            state = self.try_dequeue();
        }
        unsafe { state.unwrap_unchecked() }
    }

    pub fn try_dequeue(&self) -> DequeueResult<T> {
        if self
            .being_dequeued
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return DequeueResult::InUse;
        }

        let mut res = unsafe { self.dequeue_impl() };
        // If we are being asked to retry, we try again once.
        if matches!(res, DequeueResult::Retry) {
            res = unsafe { self.dequeue_impl() };
        }

        self.being_dequeued.store(false, Ordering::Release);

        res
    }

    unsafe fn dequeue_impl(&self) -> DequeueResult<T> {
        let tail_ptr = self.tail.get();

        let Some(mut tail_node) = NonNull::new(unsafe { *tail_ptr }) else {
            return DequeueResult::Element(None);
        };
        let mut next = unsafe { T::get_link(tail_node).as_ref() }
            .next
            .load(Ordering::Acquire);

        if tail_node == self.stub {
            let Some(next_node) = NonNull::new(next) else {
                return DequeueResult::Element(None);
            };

            unsafe { *tail_ptr = next };
            tail_node = next_node;
            next = unsafe { T::get_link(tail_node).as_ref() }
                .next
                .load(Ordering::Acquire);
        }

        if !next.is_null() {
            unsafe { *tail_ptr = next };
            return DequeueResult::Element(Some(unsafe { T::capture(tail_node) }));
        }

        let head = self.head.load(Ordering::Acquire);

        if tail_node.as_ptr() != head {
            // Another thread is operating on the queue.
            // We should give up and retry in a short while.
            return DequeueResult::Retry;
        }

        unsafe { self.enqueue_ptr(self.stub) };

        next = unsafe { T::get_link(tail_node).as_ref() }
            .next
            .load(Ordering::Acquire);
        if next.is_null() {
            return DequeueResult::Element(None);
        }

        unsafe { *tail_ptr = next };

        DequeueResult::Element(Some(unsafe { T::capture(tail_node) }))
    }
}

impl<T: Queueable> Drop for MpscQueue<T> {
    fn drop(&mut self) {
        let mut current = unsafe { *self.tail.get() };

        while let Some(node) = NonNull::new(current) {
            let next = unsafe { T::get_link(node).as_ref() }
                .next
                .load(Ordering::Relaxed);

            drop(unsafe { T::capture(node) });

            current = next;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Barrier};
    use std::thread::spawn;

    extern crate alloc;
    use alloc::boxed::Box;
    use core::{mem::offset_of, pin::Pin};

    struct Element {
        value: u8,
        next: Option<NonNull<Element>>,
    }

    impl Unpin for Element {}

    impl Queueable for Element {
        type Handle = Pin<Box<Self>>;

        fn release(r: Self::Handle) -> NonNull<Self> {
            let ptr = Box::into_raw(Pin::into_inner(r));
            unsafe { NonNull::new_unchecked(ptr) }
        }

        unsafe fn capture(ptr: NonNull<Self>) -> Self::Handle {
            Pin::new(unsafe { Box::from_raw(ptr.as_ptr()) })
        }

        unsafe fn get_link(ptr: NonNull<Self>) -> NonNull<Link<Self>> {
            let base = ptr.as_ptr().cast::<Link<Self>>();
            let ptr = unsafe { base.byte_add(offset_of!(Element, next)) };
            unsafe { NonNull::new_unchecked(ptr) }
        }
    }

    #[test]
    fn test_mpsc_queue() {
        let queue: MpscQueue<Element> = MpscQueue::new(Box::pin(Element {
            value: 0,
            next: None,
        }));
        queue.enqueue(Box::pin(Element {
            value: 1,
            next: None,
        }));
        queue.enqueue(Box::pin(Element {
            value: 2,
            next: None,
        }));
        let element1 = queue.dequeue().unwrap();
        let element2 = queue.dequeue().unwrap();
        assert_eq!(element1.value, 1);
        assert_eq!(element2.value, 2);
    }

    #[cfg(miri)]
    #[test]
    fn test_mpsc_drop() {
        let queue: MpscQueue<Element> = MpscQueue::new(Box::pin(Element {
            value: 0,
            next: None,
        }));
        queue.enqueue(Box::pin(Element {
            value: 1,
            next: None,
        }));
        queue.enqueue(Box::pin(Element {
            value: 2,
            next: None,
        }));
    }

    #[test]
    fn test_concurent() {
        let num_threads = 10;

        let queue = Arc::new(MpscQueue::<Element>::new(Box::pin(Element {
            value: 0,
            next: None,
        })));
        let barrier = Arc::new(Barrier::new(num_threads));

        let mut handles = Vec::with_capacity(num_threads);

        for _ in 0..num_threads {
            handles.push(spawn({
                let queue = queue.clone();
                let barrier = barrier.clone();
                move || {
                    barrier.wait();
                    queue.enqueue(Box::pin(Element {
                        value: 42,
                        next: None,
                    }));
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let mut handles = Vec::with_capacity(num_threads);

        for _ in 0..num_threads {
            let queue = queue.clone();
            let barrier = barrier.clone();
            handles.push(spawn(move || {
                barrier.wait();
                let element = queue.dequeue().unwrap();
                assert_eq!(element.value, 42);
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        assert!(queue.dequeue().is_none());
    }
}
