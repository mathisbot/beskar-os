//! Memory mapping abstractions for generic ELF loading.
pub use beskar_core::arch::VirtAddr;

/// Page flags for memory protection and access control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageFlags(u8);

impl PageFlags {
    const PRESENT: u8 = 0b0001;
    const WRITABLE: u8 = 0b0010;
    const EXECUTABLE: u8 = 0b0100;
    const USER_ACCESSIBLE: u8 = 0b1000;

    #[must_use]
    #[inline]
    /// Create read-only executable flags
    pub const fn rx() -> Self {
        Self(Self::PRESENT | Self::EXECUTABLE)
    }
    #[must_use]
    #[inline]
    /// Create read-write executable flags
    pub const fn rwx() -> Self {
        Self(Self::PRESENT | Self::WRITABLE | Self::EXECUTABLE)
    }
    #[must_use]
    #[inline]
    /// Create read-only data flags
    pub const fn r() -> Self {
        Self(Self::PRESENT)
    }
    #[must_use]
    #[inline]
    /// Create read-write data flags
    pub const fn rw() -> Self {
        Self(Self::PRESENT | Self::WRITABLE)
    }

    #[must_use]
    #[inline]
    pub const fn is_present(&self) -> bool {
        (self.0 & Self::PRESENT) != 0
    }
    #[must_use]
    #[inline]
    pub const fn is_writable(&self) -> bool {
        (self.0 & Self::WRITABLE) != 0
    }
    #[must_use]
    #[inline]
    pub const fn is_executable(&self) -> bool {
        (self.0 & Self::EXECUTABLE) != 0
    }
    #[must_use]
    #[inline]
    pub const fn is_user_accessible(&self) -> bool {
        (self.0 & Self::USER_ACCESSIBLE) != 0
    }

    #[must_use]
    #[inline]
    /// Set user accessibility
    pub const fn set_user_accessible(mut self, user_accessible: bool) -> Self {
        if user_accessible {
            self.0 |= Self::USER_ACCESSIBLE;
        } else {
            self.0 &= !Self::USER_ACCESSIBLE;
        }
        self
    }
}

/// Information about a mapped memory region.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MappedRegion {
    /// Virtual address of the region
    pub virt_addr: VirtAddr,
    /// Size of the region in bytes
    pub size: u64,
}

#[expect(clippy::result_unit_err)]
/// Abstract interface for memory mapping operations.
pub trait MemoryMapper {
    /// Map a contiguous virtual address range with given flags.
    /// Returns the virtual address of the mapped region.
    ///
    /// # Errors
    ///
    /// Returns `Err(())` if the mapping fails.
    fn map_region(&mut self, size: u64, flags: PageFlags)
    -> core::result::Result<MappedRegion, ()>;

    /// Copy data into a mapped region.
    ///
    /// # Errors
    ///
    /// Returns `Err(())` if the copy fails.
    fn copy_data(&mut self, dest: VirtAddr, src: &[u8]) -> core::result::Result<(), ()>;

    /// Zero a memory region.
    ///
    /// # Errors
    ///
    /// Returns `Err(())` if the operation fails.
    fn zero_region(&mut self, dest: VirtAddr, size: u64) -> core::result::Result<(), ()>;

    /// Update page flags for a memory region.
    ///
    /// # Errors
    ///
    /// Returns `Err(())` if the update fails.
    fn update_flags(
        &mut self,
        region: MappedRegion,
        flags: PageFlags,
    ) -> core::result::Result<(), ()>;

    /// Unmap a region and free its resources.
    ///
    /// # Errors
    ///
    /// Returns `Err(())` if the unmapping fails.
    fn unmap_region(&mut self, region: MappedRegion) -> core::result::Result<(), ()>;

    /// Abort and rollback all mappings created so far.
    ///
    /// This is a best-effort operation and may not guarantee complete cleanup.
    fn rollback(&mut self);
}

#[cfg(test)]
mod tests {
    use super::{MappedRegion, MemoryMapper, PageFlags, VirtAddr};
    use alloc::vec::Vec;

    #[derive(Debug, Default)]
    /// Mock memory mapper for testing
    struct MockMapper {
        regions: Vec<(VirtAddr, u64)>,
    }

    impl MemoryMapper for MockMapper {
        fn map_region(
            &mut self,
            size: u64,
            _flags: PageFlags,
        ) -> core::result::Result<MappedRegion, ()> {
            let virt_addr = if self.regions.is_empty() {
                VirtAddr::new_extend(0x1000)
            } else {
                let (last_start, last_size) = self.regions.last().unwrap();
                *last_start + *last_size
            };

            self.regions.push((virt_addr, size));

            Ok(MappedRegion { virt_addr, size })
        }

        fn copy_data(&mut self, _dest: VirtAddr, _src: &[u8]) -> core::result::Result<(), ()> {
            Ok(())
        }

        fn zero_region(&mut self, _dest: VirtAddr, _size: u64) -> core::result::Result<(), ()> {
            Ok(())
        }

        fn update_flags(
            &mut self,
            _region: MappedRegion,
            _flags: PageFlags,
        ) -> core::result::Result<(), ()> {
            Ok(())
        }

        fn unmap_region(&mut self, _region: MappedRegion) -> core::result::Result<(), ()> {
            Ok(())
        }

        fn rollback(&mut self) {}
    }

    #[test]
    fn test_page_flags_creation() {
        let rw = PageFlags::rw();
        assert!(rw.is_present());
        assert!(rw.is_writable());
        assert!(!rw.is_executable());
        assert!(!rw.is_user_accessible());

        let rx = PageFlags::rx().set_user_accessible(true);
        assert!(rx.is_present());
        assert!(!rx.is_writable());
        assert!(rx.is_executable());
        assert!(rx.is_user_accessible());
    }

    #[test]
    fn test_mock_mapper() {
        let mut mapper = MockMapper::default();

        let region1 = mapper.map_region(0x1000, PageFlags::rw()).unwrap();
        assert_eq!(region1.virt_addr.as_u64(), 0x1000);
        assert_eq!(region1.size, 0x1000);

        let region2 = mapper.map_region(0x2000, PageFlags::rx()).unwrap();
        assert_eq!(region2.virt_addr.as_u64(), 0x2000);
        assert_eq!(region2.size, 0x2000);

        assert_eq!(mapper.regions.len(), 2);
    }
}
