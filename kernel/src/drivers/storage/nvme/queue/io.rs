use super::{CompletionQueue, SubmissionQueue};
use beskar_core::arch::PhysAddr;
use beskar_core::drivers::DriverResult;
use hyperdrive::ptrs::volatile::{ReadWrite, Volatile};

pub struct IoCompletionQueue(CompletionQueue);

impl IoCompletionQueue {
    #[inline]
    pub fn new(doorbell: Volatile<ReadWrite, u32>) -> DriverResult<Self> {
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
}

pub struct IoSubmissionQueue(SubmissionQueue);

impl IoSubmissionQueue {
    #[inline]
    pub fn new(doorbell: Volatile<ReadWrite, u32>) -> DriverResult<Self> {
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
}
