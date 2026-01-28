use super::{CompletionQueue, SubmissionQueue};
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
}
