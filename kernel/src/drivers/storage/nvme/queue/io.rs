use super::{CompletionEntry, CompletionQueue, SubmissionEntry, SubmissionQueue};
use beskar_core::arch::PhysAddr;
use beskar_core::drivers::DriverResult;
use driver_shared::mmio::MmioRegister;
use hyperdrive::ptrs::volatile::ReadWrite;

pub struct IoCompletionQueue(CompletionQueue);

impl IoCompletionQueue {
    #[inline]
    pub fn new(doorbell: MmioRegister<ReadWrite, u32>) -> DriverResult<Self> {
        Ok(Self(CompletionQueue::new(doorbell)?))
    }
    #[must_use]
    #[inline]
    pub const fn paddr(&self) -> PhysAddr {
        self.0.paddr()
    }
    #[must_use]
    #[inline]
    pub const fn entries(&self) -> u16 {
        self.0.entries()
    }

    #[inline]
    pub fn pop(&mut self) -> Option<IoCompletionEntry> {
        self.0.pop().map(IoCompletionEntry)
    }
}

pub struct IoSubmissionQueue(SubmissionQueue);

impl IoSubmissionQueue {
    #[inline]
    pub fn new(doorbell: MmioRegister<ReadWrite, u32>) -> DriverResult<Self> {
        Ok(Self(SubmissionQueue::new(doorbell)?))
    }
    #[must_use]
    #[inline]
    pub const fn paddr(&self) -> PhysAddr {
        self.0.paddr()
    }
    #[must_use]
    #[inline]
    pub const fn entries(&self) -> u16 {
        self.0.entries()
    }

    #[inline]
    pub fn push(&mut self, entry: &IoSubmissionEntry) {
        self.0.push(entry.0);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Command {
    Write = 0x01,
    Read = 0x02,
}

pub struct IoSubmissionEntry(SubmissionEntry);

impl IoSubmissionEntry {
    #[must_use]
    #[inline]
    pub fn new_read(nsid: u32, slba: u64, blocks: u16, buffer: PhysAddr) -> Self {
        Self::new_rw(Command::Read, nsid, slba, blocks, buffer)
    }

    #[must_use]
    #[inline]
    pub fn new_write(nsid: u32, slba: u64, blocks: u16, buffer: PhysAddr) -> Self {
        Self::new_rw(Command::Write, nsid, slba, blocks, buffer)
    }

    #[must_use]
    fn new_rw(command: Command, nsid: u32, slba: u64, blocks: u16, buffer: PhysAddr) -> Self {
        let mut entry = SubmissionEntry::zero_with_opcode(command as u8);
        entry.nsid = nsid;
        entry.data_ptr[0] = buffer;
        entry.command_specific[0] = (slba & 0xFFFF_FFFF) as u32;
        entry.command_specific[1] = ((slba >> 32) & 0xFFFF_FFFF) as u32;
        entry.command_specific[2] = u32::from(blocks - 1);
        Self(entry)
    }

    #[must_use]
    #[inline]
    pub fn command_id(&self) -> u16 {
        self.0.command_id().as_u16()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IoCompletionEntry(CompletionEntry);

impl IoCompletionEntry {
    #[must_use]
    #[inline]
    pub const fn is_success(self) -> bool {
        self.0.is_success()
    }

    #[must_use]
    #[inline]
    pub const fn command_id(self) -> u16 {
        self.0.command_id().as_u16()
    }

    #[must_use]
    #[inline]
    pub const fn status_code(self) -> u16 {
        self.0.status_code()
    }
}
