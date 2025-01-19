use crate::arch::commons::paging::{CacheFlush, MemSize, Page};

pub mod page_table;

pub struct TlbFlush<S: MemSize>(Page<S>);

impl<S: MemSize> TlbFlush<S> {
    #[inline]
    pub fn new(page: Page<S>) -> Self {
        Self(page)
    }

    #[inline]
    pub fn flush(&self) {
        unsafe {
            core::arch::asm!("invlpg [{}]", in(reg) self.0.start_address().as_u64(), options(nostack, preserves_flags));
        }
    }

    #[inline]
    /// Ignore the flush operation on the TLB.
    ///
    /// ## Safety
    ///
    /// The page table containing the page must not be used at the moment,
    /// otherwise the CPU will not be aware of the changes.
    pub unsafe fn ignore_flush(self) {}

    #[must_use]
    #[inline]
    pub fn page(&self) -> Page<S> {
        self.0
    }
}

impl<S: MemSize> CacheFlush<S> for TlbFlush<S> {
    fn new(page: Page<S>) -> Self {
        Self::new(page)
    }

    #[inline]
    fn flush(&self) {
        self.flush();
    }

    #[inline]
    unsafe fn ignore_flush(self) {
        unsafe { self.ignore_flush() };
    }

    #[inline]
    fn page(&self) -> Page<S> {
        self.page()
    }
}
