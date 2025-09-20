use beskar_core::arch::paging::{CacheFlush, MemSize, Page};

pub mod page_table;

pub struct TlbFlush<S: MemSize>(Page<S>);

impl<S: MemSize> TlbFlush<S> {
    #[must_use]
    #[inline]
    pub const fn new(page: Page<S>) -> Self {
        Self(page)
    }

    #[inline]
    pub fn flush(&self) {
        unsafe {
            core::arch::asm!("invlpg [{}]", in(reg) self.0.start_address().as_u64(), options(nostack, nomem, preserves_flags));
        }
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
        self.flush();
    }

    #[inline]
    fn page(&self) -> Page<S> {
        self.page()
    }
}
