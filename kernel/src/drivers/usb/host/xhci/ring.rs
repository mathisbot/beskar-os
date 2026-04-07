use super::trb::{LinkTrb, Trb};
use crate::mem::vmm;
use alloc::vec::Vec;
use beskar_core::arch::{
    PhysAddr, VirtAddr,
    paging::{Frame, M4KiB, MemSize as _, Page},
};
use beskar_hal::paging::page_table::Flags;

pub(super) trait RingElement {
    /// Set the cycle bit
    fn set_cycle_bit(&mut self, cycle_bit: bool);
}

/// Ring structure for xHCI
///
/// Rings are circular buffers of TRBs.
#[derive(Debug)]
pub struct Ring<T: RingElement + Sized> {
    /// Virtual address of the ring buffer
    vaddr: VirtAddr,
    /// Physical address of the ring buffer
    paddr: PhysAddr,
    /// Number of TRBs in the ring
    capacity: u8,
    /// Current producer cycle state
    cycle_bit: bool,
    /// Current producer index
    enqueue_index: u8,
    /// Current consumer index
    dequeue_index: u8,
    /// Backing virtual page in the kernel-global pool
    page: Page<M4KiB>,
    /// Backing physical frame owned by this ring
    frame: Frame<M4KiB>,
    _phantom: core::marker::PhantomData<T>,
}

impl<T: RingElement + Sized> Ring<T> {
    /// Create a new ring
    ///
    /// # Panics
    ///
    /// Panics if memory allocation fails or if `capacity` is 0.
    #[must_use]
    pub fn new(capacity: u8) -> Self {
        assert!(capacity > 0, "Ring capacity must be greater than 0");
        assert!(usize::from(capacity) * size_of::<T>() <= usize::try_from(M4KiB::SIZE).unwrap());

        let page = vmm::kernel::reserve_pages::<M4KiB>(1).unwrap().start();
        let frame = vmm::kernel::alloc_frame::<M4KiB>().unwrap();

        let flags = Flags::MMIO_SUITABLE | Flags::WRITABLE;
        vmm::kernel::map_frame(page, frame, flags).unwrap();

        let vaddr = page.start_address();

        // Initialize the ring
        let ring = Self {
            vaddr,
            paddr: frame.start_address(),
            capacity,
            cycle_bit: true,
            enqueue_index: 0,
            dequeue_index: 0,
            page,
            frame,
            _phantom: core::marker::PhantomData,
        };

        // Zeroize the ring
        unsafe {
            core::ptr::write_bytes(ring.vaddr.as_mut_ptr::<T>(), 0, usize::from(capacity));
        }

        ring
    }

    /// Get the physical address of the ring
    #[must_use]
    #[inline]
    pub const fn phys_addr(&self) -> PhysAddr {
        self.paddr
    }

    /// Get the virtual address of the ring
    #[must_use]
    #[inline]
    pub const fn virt_addr(&self) -> VirtAddr {
        self.vaddr
    }

    /// Get the capacity of the ring
    #[must_use]
    #[inline]
    pub const fn capacity(&self) -> u8 {
        self.capacity
    }

    /// Get the current producer cycle state
    #[must_use]
    #[inline]
    pub const fn cycle_bit(&self) -> bool {
        self.cycle_bit
    }

    /// Get the current producer index
    #[must_use]
    #[inline]
    pub const fn enqueue_index(&self) -> u8 {
        self.enqueue_index
    }

    /// Get a reference to a TRB at the specified index
    #[must_use]
    pub fn trb(&self, index: u8) -> &T {
        assert!(index < self.capacity, "Index out of bounds");
        unsafe { &*self.vaddr.as_ptr::<T>().add(usize::from(index)) }
    }

    #[must_use]
    /// Get a mutable reference to a TRB at the specified index
    pub fn trb_mut(&mut self, index: u8) -> &mut T {
        assert!(index < self.capacity, "Index out of bounds");
        unsafe { &mut *self.vaddr.as_mut_ptr::<T>().add(usize::from(index)) }
    }

    /// Push a TRB to the ring
    ///
    /// Returns the index of the pushed TRB
    pub fn push(&mut self, mut trb: T) -> u8 {
        // Set the cycle bit
        trb.set_cycle_bit(self.cycle_bit);

        // Write the TRB to the ring
        let index = self.enqueue_index;
        *self.trb_mut(index) = trb;

        // Update the enqueue index
        self.enqueue_index = (self.enqueue_index + 1) % self.capacity;

        // If we've wrapped around, toggle the cycle bit
        if self.enqueue_index == 0 {
            self.cycle_bit = !self.cycle_bit;
        }

        index
    }

    pub fn pop(&mut self) -> Option<T> {
        // Check if the ring is empty
        if self.enqueue_index == self.dequeue_index {
            return None;
        }

        // Read the TRB from the ring
        let trb = unsafe { core::ptr::read(self.trb(self.dequeue_index)) };

        // Update the dequeue index
        self.dequeue_index = (self.dequeue_index + 1) % self.capacity;

        Some(trb)
    }
}

impl<T: RingElement + Sized> Drop for Ring<T> {
    fn drop(&mut self) {
        if let Ok(frame) = vmm::kernel::unmap_page(self.page) {
            vmm::kernel::free_frame(frame);
        } else {
            vmm::kernel::free_frame(self.frame);
        }
        vmm::kernel::free_pages(Page::range_inclusive(self.page, self.page));
    }
}

/// Command Ring
///
/// A ring used to send commands to the xHCI controller.
/// See xHCI spec section 4.9.3
#[derive(Debug)]
pub struct CommandRing {
    /// The underlying ring
    ring: Ring<Trb>,
}

impl CommandRing {
    /// Create a new command ring
    ///
    /// # Panics
    ///
    /// Panics if memory allocation fails
    #[must_use]
    pub fn new(capacity: u8) -> Self {
        // Command rings should have a link TRB at the end
        let mut ring = Ring::new(capacity);

        // Add a link TRB at the end
        let link_trb = LinkTrb::new(ring.phys_addr().as_u64(), true);
        let index = ring.capacity() - 1;
        *ring.trb_mut(index) = *link_trb.as_trb();

        Self { ring }
    }

    #[must_use]
    #[inline]
    pub const fn phys_addr(&self) -> PhysAddr {
        self.ring.phys_addr()
    }
}

/// Event Ring Segment
///
/// A segment of an event ring.
#[derive(Debug)]
pub struct EventRingSegment {
    /// The underlying ring buffer
    ring: Ring<Trb>,
}

impl EventRingSegment {
    /// Create a new event ring segment
    ///
    /// # Panics
    ///
    /// Panics if memory allocation fails
    #[must_use]
    #[inline]
    pub fn new(capacity: u8) -> Self {
        let ring = Ring::new(capacity);

        Self { ring }
    }

    #[must_use]
    #[inline]
    pub const fn phys_addr(&self) -> PhysAddr {
        self.ring.phys_addr()
    }
}

/// Event Ring Segment Table Entry
///
/// An entry in the Event Ring Segment Table.
/// See xHCI spec section 6.5
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct EventRingSegmentTableEntry {
    // TODO:
}

impl EventRingSegmentTableEntry {
    #[must_use]
    #[inline]
    pub const fn new(_segment: &EventRingSegment) -> Self {
        Self {}
    }
}

/// Event Ring
///
/// A ring used to receive events from the xHCI controller.
/// See xHCI spec section 4.9.4
#[derive(Debug)]
pub struct EventRing {
    /// Segments of the event ring
    segments: Vec<EventRingSegment>,
    /// Event Ring Segment Table
    segment_table: &'static mut [EventRingSegmentTableEntry],
    /// Physical address of the segment table
    segment_table_paddr: PhysAddr,
    /// Segment table backing page
    segment_table_page: Page<M4KiB>,
    /// Segment table backing frame
    segment_table_frame: Frame<M4KiB>,
    /// Current consumer index
    dequeue_index: usize,
    /// Current segment index
    segment_index: usize,
}

impl EventRing {
    /// Create a new event ring with a single segment
    ///
    /// # Panics
    ///
    /// Panics if memory allocation fails
    #[must_use]
    pub fn new(capacity: u8) -> Self {
        // Create a single segment
        let segment = EventRingSegment::new(capacity);
        let segments = alloc::vec![segment];

        let flags = Flags::MMIO_SUITABLE;
        let table_size = size_of::<EventRingSegmentTableEntry>();
        assert!(
            table_size <= usize::try_from(M4KiB::SIZE).unwrap(),
            "Segment table size exceeds page size"
        );
        let page = vmm::kernel::reserve_pages::<M4KiB>(1).unwrap().start();
        let frame = vmm::kernel::alloc_frame::<M4KiB>().unwrap();
        vmm::kernel::map_frame(page, frame, flags).unwrap();

        let virt_addr = page.start_address();

        // Initialize the segment table
        let segment_table = unsafe { core::slice::from_raw_parts_mut(virt_addr.as_mut_ptr(), 1) };
        segment_table[0] = EventRingSegmentTableEntry::new(&segments[0]);

        Self {
            segments,
            segment_table,
            segment_table_paddr: frame.start_address(),
            segment_table_page: page,
            segment_table_frame: frame,
            dequeue_index: 0,
            segment_index: 0,
        }
    }

    #[must_use]
    #[inline]
    /// Get the physical address of the segment table
    pub const fn segment_table_phys_addr(&self) -> PhysAddr {
        self.segment_table_paddr
    }

    #[must_use]
    #[inline]
    /// Get the number of entries in the segment table
    pub const fn segment_table_size(&self) -> usize {
        self.segment_table.len()
    }

    #[must_use]
    #[inline]
    /// Get the current consumer index
    pub const fn dequeue_index(&self) -> usize {
        self.dequeue_index
    }

    #[must_use]
    #[inline]
    /// Get the current segment index
    pub const fn segment_index(&self) -> usize {
        self.segment_index
    }

    #[must_use]
    #[inline]
    /// Get the current segment
    pub fn current_segment(&self) -> &EventRingSegment {
        &self.segments[self.segment_index]
    }
}

impl Drop for EventRing {
    fn drop(&mut self) {
        if let Ok(frame) = vmm::kernel::unmap_page(self.segment_table_page) {
            vmm::kernel::free_frame(frame);
        } else {
            vmm::kernel::free_frame(self.segment_table_frame);
        }
        vmm::kernel::free_pages(Page::range_inclusive(
            self.segment_table_page,
            self.segment_table_page,
        ));
    }
}

/// Transfer Ring
///
/// A ring used to send transfer requests to the xHCI controller.
/// See xHCI spec section 4.9.2
#[derive(Debug)]
pub struct TransferRing {
    /// The underlying ring
    ring: Ring<Trb>,
}

impl TransferRing {
    /// Create a new transfer ring
    ///
    /// # Panics
    ///
    /// Panics if memory allocation fails
    #[must_use]
    pub fn new(capacity: u8) -> Self {
        let mut ring = Ring::new(capacity);

        // Transfer rings should have a link TRB at the end
        let link_trb = LinkTrb::new(ring.phys_addr().as_u64(), true);
        let index = ring.capacity() - 1;
        *ring.trb_mut(index) = *link_trb.as_trb();

        Self { ring }
    }
}
