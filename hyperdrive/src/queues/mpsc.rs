//! An intrusive, multiple-producer single-consumer queue.
//!
//! In order to be used, the element type must implement the `Queueable` trait.
//!
//! ## `Queueable`
//!
//! As the queue is intrusive, the element type must provide a way to link the elements together.
//! Also, as the queue cannot deal with implementation details (such as getting a pointer out of
//! a pinned `Box`), the element type must provide a way to capture and release the data.
//!
//! ### `Handle`
//!
//! The `Handle` type is the type that owns the data.
//! Most of the time, it is `Pin<Box<Self>`.
//!
//! ### Example
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
//! ```
//!
//! ### About `Link`
//!
//! `Link` is the API that the queue uses to link the elements together.
//!
//! While it is a bit more complex on the inside, it behaves like a simple pointer to the next element.
//! This is why it is possible to write `get_link` the way it is in the above example.
//!
//! ## Usage
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
//! # struct Element {
//! #     value: u8,
//! #     next: Option<NonNull<Element>>,
//! # }
//! #
//! # impl Unpin for Element {}
//! #
//! # impl Queueable for Element {
//! #     type Handle = Pin<Box<Self>>;
//! #
//! #     fn release(r: Self::Handle) -> NonNull<Self> {
//! #         let ptr = Box::into_raw(Pin::into_inner(r));
//! #         unsafe { NonNull::new_unchecked(ptr) }
//! #     }
//! #     
//! #     unsafe fn capture(ptr: NonNull<Self>) -> Self::Handle {
//! #         Pin::new(unsafe { Box::from_raw(ptr.as_ptr()) })
//! #     }
//! #
//! #     unsafe fn get_link(ptr: NonNull<Self>) -> NonNull<Link<Self>> {
//! #         let base = ptr.as_ptr().cast::<Link<Self>>();
//! #         let ptr = unsafe { base.byte_add(offset_of!(Element, next)) };
//! #         unsafe { NonNull::new_unchecked(ptr) }
//! #     }
//! # }
//! #
//! let queue: MpscQueue<Element> = MpscQueue::new(Box::pin(Element { value: 0, next: None }));
//!
//! queue.enqueue(Box::pin(Element { value: 1, next: None }));
//!
//! let element = queue.dequeue().unwrap();
//! assert_eq!(element.value, 1);
//! ```
use core::{
    ptr::{self, NonNull},
    sync::atomic::{AtomicBool, AtomicPtr, Ordering},
};

pub trait Queueable: Sized {
    /// `Handle` is the type that owns the data.
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

#[repr(transparent)]
/// Describes a link between two elements in the queue.
pub struct Link<T> {
    /// The next element in the queue.
    next: AtomicPtr<T>,
    /// A phantom field to pin the link.
    _pin: core::marker::PhantomPinned,
}

impl<T> Default for Link<T> {
    fn default() -> Self {
        Self {
            next: AtomicPtr::new(ptr::null_mut()),
            _pin: core::marker::PhantomPinned,
        }
    }
}

/// A multiple-producer single-consumer queue.
pub struct MpscQueue<T: Queueable> {
    /// The head of the queue.
    head: AtomicPtr<T>,
    /// The tail of the queue.
    tail: AtomicPtr<T>,
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
    /// The queue is busy.
    InUse,
}

impl<T: Queueable> DequeueResult<T> {
    #[must_use]
    /// Unwraps the result.
    ///
    /// ## Panics
    ///
    /// Panics if the result is not of type `Element`.
    pub fn unwrap(self) -> Option<T::Handle> {
        match self {
            Self::Element(res) => res,
            Self::InUse => panic!("Unwrapped a DequeueResult::InUse"),
        }
    }

    #[must_use]
    /// Unwraps the result without checking its value.
    ///
    /// ## Safety
    ///
    /// The caller must ensure that the result is of type `Element`.
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
            tail: AtomicPtr::new(stub_ptr.as_ptr()),
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
        loop {
            match self.try_dequeue() {
                DequeueResult::Element(e) => break e,
                DequeueResult::InUse => core::hint::spin_loop(),
            }
        }
    }

    pub fn try_dequeue(&self) -> DequeueResult<T> {
        if self
            .being_dequeued
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return DequeueResult::InUse;
        }

        let res = unsafe { self.dequeue_impl() };

        self.being_dequeued.store(false, Ordering::Release);

        DequeueResult::Element(res)
    }

    /// ## Safety
    ///
    /// The caller must make sure that the queue is not being dequeued by another thread.
    unsafe fn dequeue_impl(&self) -> Option<T::Handle> {
        let mut tail_node = unsafe { NonNull::new_unchecked(self.tail.load(Ordering::Relaxed)) };
        let mut next = unsafe { T::get_link(tail_node).as_ref() }
            .next
            .load(Ordering::Acquire);

        // If node is the stub, dequeue it and use the next one
        if tail_node == self.stub {
            let next_node = NonNull::new(next)?;

            self.tail.store(next, Ordering::Relaxed);
            tail_node = next_node;
            next = unsafe { T::get_link(tail_node).as_ref() }
                .next
                .load(Ordering::Acquire);
        }

        // If there is a next node, simply cycle the queue
        if !next.is_null() {
            self.tail.store(next, Ordering::Relaxed);
            return Some(unsafe { T::capture(tail_node) });
        }

        // Otherwise, enqueue the stub, and then there is a next node!
        unsafe { self.enqueue_ptr(self.stub) };

        // We still have to check the next node because it is possible that
        // another node has been enqueued before the stub.
        // It is also possible that the enqueueing thread hasn't had time to update the next pointer.
        // We need to wait for it (should be rare).
        let next_link = unsafe { T::get_link(tail_node).as_ref() };
        let next = loop {
            let next = next_link.next.load(Ordering::Acquire);
            if !next.is_null() {
                break next;
            }
            core::hint::spin_loop();
        };

        self.tail.store(next, Ordering::Relaxed);

        Some(unsafe { T::capture(tail_node) })
    }
}

impl<T: Queueable> Drop for MpscQueue<T> {
    fn drop(&mut self) {
        let mut current = self.tail.load(Ordering::Relaxed);

        while let Some(node) = NonNull::new(current) {
            let next = unsafe { T::get_link(node).as_ref() }
                .next
                .load(Ordering::Relaxed);

            if node != self.stub {
                drop(unsafe { T::capture(node) });
            }

            current = next;
        }

        // The stub isn't necessarily in the queue, so we have to drop it manually
        drop(unsafe { T::capture(self.stub) });
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
        assert!(queue.dequeue().is_none());
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
        assert!(queue.dequeue().is_none());
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
    fn test_mpsc_concurent() {
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

    #[test]
    fn test_mpsc_concurent_interlaced() {
        let num_threads = 2 * 5;

        let queue = Arc::new(MpscQueue::<Element>::new(Box::pin(Element {
            value: 0,
            next: None,
        })));

        for _ in 0..num_threads / 2 + 1 {
            queue.enqueue(Box::pin(Element {
                value: 42,
                next: None,
            }));
        }

        let barrier = Arc::new(Barrier::new(num_threads));
        let mut handles = Vec::with_capacity(num_threads);

        for i in 0..num_threads {
            handles.push(spawn({
                let queue = queue.clone();
                let barrier = barrier.clone();
                move || {
                    barrier.wait();
                    if i % 2 == 0 {
                        assert_eq!(queue.dequeue().unwrap().value, 42);
                    } else {
                        queue.enqueue(Box::pin(Element {
                            value: 1,
                            next: None,
                        }));
                    }
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }
}
