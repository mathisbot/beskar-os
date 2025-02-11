use core::{
    ptr::NonNull,
    sync::atomic::{AtomicU16, Ordering},
};

use beskar_core::{
    arch::commons::{
        PhysAddr,
        paging::{Flags, M4KiB},
    },
    drivers::{DriverError, DriverResult},
};
use hyperdrive::volatile::{ReadWrite, Volatile};

use crate::mem::{frame_alloc, page_alloc::pmap::PhysicalMapping};

pub mod admin;
pub mod io;

struct Queue<T> {
    base: Volatile<ReadWrite, T>,
    pmap: PhysicalMapping,
    size: u16,
    tail: u16,
    head: u16,
}

impl<T> Queue<T> {
    fn new() -> DriverResult<Self> {
        let Some(frame) =
            frame_alloc::with_frame_allocator(frame_alloc::FrameAllocator::alloc::<M4KiB>)
        else {
            return Err(DriverError::Unknown);
        };

        let flags = Flags::MMIO_SUITABLE;
        let pmap = PhysicalMapping::new(
            frame.start_address(),
            frame.size().try_into().unwrap(),
            flags,
        );
        let base = pmap.translate(frame.start_address()).unwrap();

        Ok(Self {
            base: Volatile::new(NonNull::new(base.as_mut_ptr()).unwrap()),
            pmap,
            size: u16::try_from(frame.size()).unwrap(),
            tail: 0,
            head: 0,
        })
    }
}

impl<T> Drop for Queue<T> {
    fn drop(&mut self) {
        let frame = self.pmap.start_frame();
        frame_alloc::with_frame_allocator(|fralloc| fralloc.free(frame));
    }
}

struct SubmissionQueue(Queue<SubmissionEntry>);

impl SubmissionQueue {
    #[inline]
    pub fn new() -> DriverResult<Self> {
        Ok(Self(Queue::new()?))
    }

    #[must_use]
    #[inline]
    pub const fn paddr(&self) -> PhysAddr {
        self.0.pmap.start_frame().start_address()
    }

    #[must_use]
    #[inline]
    const fn is_full(&self) -> bool {
        self.0.tail.wrapping_add(1) % self.0.size == self.0.head
    }

    /// Push a new entry to the queue
    ///
    /// ## Warning
    ///
    /// The entries are not reported to the controller until `flush` is called.
    pub fn push(&mut self, entry: SubmissionEntry) {
        // TODO: Wait for controller to empty the queue
        assert!(!self.is_full());

        let inner_queue = &mut self.0;

        let entry_ptr = unsafe { inner_queue.base.add(usize::from(inner_queue.tail)) };
        unsafe { entry_ptr.write(entry) };

        inner_queue.tail = inner_queue.tail.wrapping_add(1) % inner_queue.size;
    }

    /// Report the entries to the controller
    pub fn flush(&mut self) {
        todo!("Write to the doorbell register")
    }
}

struct CompletionQueue(Queue<CompletionEntry>);

impl CompletionQueue {
    #[inline]
    pub fn new() -> DriverResult<Self> {
        Ok(Self(Queue::new()?))
    }

    #[must_use]
    #[inline]
    pub const fn paddr(&self) -> PhysAddr {
        self.0.pmap.start_frame().start_address()
    }

    #[must_use]
    pub fn pop(&mut self) -> Option<CompletionEntry> {
        let inner_queue = &mut self.0;

        if inner_queue.head == inner_queue.tail {
            return None;
        }

        let entry_ptr = unsafe { inner_queue.base.add(usize::from(inner_queue.head)) };
        let entry = unsafe { entry_ptr.read() };

        inner_queue.head = inner_queue.head.wrapping_add(1) % inner_queue.size;

        Some(entry)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
struct SubmissionEntry {
    dword0: CommandDwordZero,
    nsid: u32,
    _reserved: [u32; 2],
    metadata_ptr: *mut u8,
    /// 2 physical addresses
    data_ptr: [*mut u8; 2],
    command_specific: [u32; 6],
}

impl SubmissionEntry {
    #[must_use]
    #[inline]
    /// Create a new submission entry with the given opcode
    ///
    /// The first DWORD (DWORD 0) is already set.
    /// Other parameters are set to 0.
    pub fn zero_with_opcode(opcode: u8) -> Self {
        Self {
            dword0: CommandDwordZero::new(opcode),
            nsid: 0,
            _reserved: [0; 2],
            metadata_ptr: core::ptr::null_mut(),
            data_ptr: [core::ptr::null_mut(); 2],
            command_specific: [0; 6],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// DWORD 0 of a command
///
/// Format:
/// - Bits 0-7: Opcode
/// - Bits 8-9: Fused operation (0 = normal operation)
/// - Bits 10-13: Reserved
/// - Bits 14-15: Physical Region Page / SGL (0 = PRP)
/// - Bits 16-31: Identifier (used in completion queue)
struct CommandDwordZero(u32);

impl CommandDwordZero {
    #[must_use]
    #[inline]
    pub fn new(opcode: u8) -> Self {
        let opcode = u32::from(opcode);
        let fused_op = 0;
        let prp = 0;
        let id = CommandIdentifier::new().as_u32();

        let value = opcode | (fused_op << 8) | (prp << 14) | (id << 16);

        Self(value)
    }

    #[must_use]
    #[inline]
    pub fn id(self) -> u16 {
        u16::try_from(self.0 >> 16).unwrap()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
struct CompletionEntry {
    command_specific: u32,
    _reserved: u32,
    s_queue_head: u16,
    s_queue_id: u16,
    /// Similar to the one in `SubmissionEntry`
    cid: u16,
    /// Status of the command
    ///
    /// Format:
    /// - Bit 0: is toggled when entry is written
    /// - Bits 1-15: 0 on success
    status: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Unique identifier for a command
///
/// Used to match completion entries to submission entries.
/// Must not be `u16::MAX`.
struct CommandIdentifier(u16);

static ID_CNTR: AtomicU16 = AtomicU16::new(0);

impl CommandIdentifier {
    #[must_use]
    #[inline]
    pub fn new() -> Self {
        let mut raw_id = ID_CNTR.fetch_add(1, Ordering::Relaxed);

        if raw_id == u16::MAX {
            ID_CNTR.store(0, Ordering::Relaxed);
            raw_id = 0;
        }

        Self(raw_id)
    }

    #[must_use]
    #[inline]
    pub fn as_u32(self) -> u32 {
        u32::from(self.0)
    }
}
