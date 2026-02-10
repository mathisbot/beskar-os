use beskar_core::arch::paging::{CacheFlush, MemSize, Page, PageRangeInclusive};

pub mod page_table;

pub struct TlbFlush<S: MemSize>(Page<S>);
pub struct TlbFlushRange<S: MemSize>(PageRangeInclusive<S>);

impl<S: MemSize> TlbFlush<S> {
    #[must_use]
    #[inline]
    pub const fn new(page: Page<S>) -> Self {
        Self(page)
    }

    #[must_use]
    #[inline]
    pub const fn page(&self) -> Page<S> {
        self.0
    }
}

impl<S: MemSize> CacheFlush<S> for TlbFlush<S> {
    #[inline]
    fn flush(&self) {
        unsafe { super::instructions::invl_pg(self.0.start_address().as_u64()) };
    }
}

impl<S: MemSize> TlbFlushRange<S> {
    #[must_use]
    #[inline]
    pub const fn new(range: PageRangeInclusive<S>) -> Self {
        Self(range)
    }

    #[must_use]
    #[inline]
    pub const fn page_range(&self) -> PageRangeInclusive<S> {
        self.0
    }
}

impl<S: MemSize> CacheFlush<S> for TlbFlushRange<S> {
    #[inline]
    fn flush(&self) {
        for page in self.0 {
            unsafe { super::instructions::invl_pg(page.start_address().as_u64()) };
        }
    }
}
