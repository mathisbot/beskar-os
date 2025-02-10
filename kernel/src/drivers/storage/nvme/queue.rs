use core::sync::atomic::{AtomicU16, Ordering};

use hyperdrive::volatile::{ReadWrite, Volatile};

pub mod admin;
pub mod io;

struct Queue<T> {
    base: Volatile<ReadWrite, T>,
    size: u16,
    tail: u16,
    head: u16,
}

struct SubmissionQueue(Queue<SubmissionEntry>);

impl SubmissionQueue {
    fn is_full(&self) -> bool {
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
    pub fn new(opcode: u8) -> Self {
        let opcode = u32::from(opcode);
        let fused_op = 0;
        let prp = 0;
        let id = u32::from(CommandIdentifier::new());

        let value = opcode | (fused_op << 8) | (prp << 14) | (id << 16);

        Self(value)
    }

    pub fn id(&self) -> u16 {
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

/// Unique identifier for a command
///
/// Used to match completion entries to submission entries.
/// Must not be `u16::MAX`.
struct CommandIdentifier(u16);

static ID_CNTR: AtomicU16 = AtomicU16::new(0);

impl CommandIdentifier {
    pub fn new() -> Self {
        let mut raw_id = ID_CNTR.fetch_add(1, Ordering::Relaxed);

        if raw_id == u16::MAX {
            ID_CNTR.store(0, Ordering::Relaxed);
            raw_id = 0;
        }

        Self(raw_id)
    }
}

impl From<CommandIdentifier> for u32 {
    fn from(id: CommandIdentifier) -> u32 {
        u32::from(id.0)
    }
}
