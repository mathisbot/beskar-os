//! Page table module.
//!
//! This only supports recursive page tables, as it is the only type of page table
//! that is used in the kernel (for now at least).

use beskar_core::arch::{
    PhysAddr, VirtAddr,
    paging::{
        Frame, FrameAllocator, M1GiB, M2MiB, M4KiB, Mapper, MappingError, MemSize, Page, Translator,
    },
};
use core::ops::{Index, IndexMut};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Flags(u64);

impl Flags {
    pub const PRESENT: Self = Self(1);
    pub const WRITABLE: Self = Self(1 << 1);
    pub const USER_ACCESSIBLE: Self = Self(1 << 2);
    pub const WRITE_THROUGH: Self = Self(1 << 3);
    pub const CACHE_DISABLED: Self = Self(1 << 4);
    pub const ACCESSED: Self = Self(1 << 5);
    pub const DIRTY: Self = Self(1 << 6);
    pub const HUGE_PAGE: Self = Self(1 << 7);
    pub const GLOBAL: Self = Self(1 << 8);
    pub const BIT_9: Self = Self(1 << 9);
    pub const NO_EXECUTE: Self = Self(1 << 63);

    pub const MMIO_SUITABLE: Self = Self(1 | (1 << 1) | (1 << 4) | (1 << 63));

    const ALL: Self = Self(0x8000_0000_0000_0FFF);
    pub const EMPTY: Self = Self(0);
    /// A set of flags that are used to mark the parent entries in the page table.
    /// The flags are present and writable.
    ///
    /// # Warning
    ///
    /// If any child page is USER ACCESSIBLE, then the parent page must also be USER ACCESSIBLE.
    const PARENT: Self = Self(1 | (1 << 1));

    #[must_use]
    #[inline]
    pub const fn as_u64(self) -> u64 {
        self.0
    }

    #[must_use]
    #[inline]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    #[must_use]
    #[inline]
    pub const fn contains(self, other: Self) -> bool {
        other.without(self).is_empty()
    }

    #[must_use]
    #[inline]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    #[must_use]
    #[inline]
    pub const fn intersection(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }

    #[must_use]
    #[inline]
    pub const fn without(self, other: Self) -> Self {
        Self(self.0 & !other.0)
    }
}

impl core::ops::BitOr for Flags {
    type Output = Self;

    #[inline]
    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}
impl core::ops::BitAnd for Flags {
    type Output = Self;

    #[inline]
    fn bitand(self, rhs: Self) -> Self::Output {
        Self(self.0 & rhs.0)
    }
}
impl core::ops::BitOrAssign for Flags {
    #[inline]
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}
impl core::ops::BitAndAssign for Flags {
    #[inline]
    fn bitand_assign(&mut self, rhs: Self) {
        self.0 &= rhs.0;
    }
}

impl beskar_core::arch::paging::Flags for Flags {}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct Entry(u64);

impl Entry {
    const FLAGS_MASK: u64 = Flags::ALL.0;
    const FRAME_MASK: u64 = 0x000F_FFFF_FFFF_F000;

    pub const EMPTY: Self = Self(0);

    #[must_use]
    #[inline]
    pub const fn as_u64(self) -> u64 {
        self.0
    }

    #[must_use]
    #[inline]
    pub const fn flags(self) -> Flags {
        Flags(self.0 & Self::FLAGS_MASK)
    }

    #[must_use]
    #[inline]
    pub const fn addr(self) -> PhysAddr {
        unsafe { PhysAddr::new_unchecked(self.0 & Self::FRAME_MASK) }
    }

    #[must_use]
    #[inline]
    pub const fn present_addr(self) -> Option<PhysAddr> {
        if self.is_present() {
            Some(self.addr())
        } else {
            None
        }
    }

    #[inline]
    pub const fn set(&mut self, addr: PhysAddr, flags: Flags) {
        debug_assert!(
            addr.as_u64() & !Self::FRAME_MASK == 0,
            "Physical address must be at least 4KiB aligned"
        );
        self.0 = (addr.as_u64() & Self::FRAME_MASK) | flags.0;
    }

    #[inline]
    /// Sets the flags, replacing the current flags while preserving the address.
    pub const fn set_flags(&mut self, flags: Flags) {
        self.0 = (self.0 & Self::FRAME_MASK) | (flags.0 & Self::FLAGS_MASK);
    }

    #[inline]
    /// Adds flags to the current flags (bitwise OR).
    pub const fn add_flags(&mut self, flags: Flags) {
        self.0 |= flags.0;
    }

    #[must_use]
    #[inline]
    pub const fn is_null(self) -> bool {
        self.0 == 0
    }

    #[must_use]
    #[inline]
    pub const fn is_present(self) -> bool {
        self.flags().contains(Flags::PRESENT)
    }

    #[must_use]
    #[inline]
    pub const fn is_large(self) -> bool {
        self.flags().contains(Flags::HUGE_PAGE)
    }

    #[must_use]
    #[inline]
    pub const fn is_user_accessible(self) -> bool {
        self.flags().contains(Flags::USER_ACCESSIBLE)
    }

    #[must_use]
    #[inline]
    pub const fn is_writable(self) -> bool {
        self.flags().contains(Flags::WRITABLE)
    }

    #[must_use]
    #[inline]
    const fn next_unchecked(raw: VirtAddr) -> VirtAddr {
        let next_raw = raw.as_u64() << 9;
        VirtAddr::new_extend(next_raw)
    }

    pub fn next<S: MemSize>(&self) -> Result<&Entries, MappingError<S>> {
        if self.is_present() && !self.is_large() {
            let va = VirtAddr::from_ptr(self);
            let next_raw = Self::next_unchecked(va);
            let entries = unsafe { &*next_raw.as_ptr() };
            Ok(entries)
        } else if self.is_present() && self.is_large() {
            Err(MappingError::UnexpectedLargePage)
        } else {
            Err(MappingError::NotMapped)
        }
    }

    pub fn next_mut<S: MemSize>(&mut self) -> Result<&mut Entries, MappingError<S>> {
        if self.is_present() && !self.is_large() {
            let va = VirtAddr::from_ptr(self);
            let next_raw = Self::next_unchecked(va);
            let entries = unsafe { &mut *next_raw.as_mut_ptr() };
            Ok(entries)
        } else if self.is_present() && self.is_large() {
            Err(MappingError::UnexpectedLargePage)
        } else {
            Err(MappingError::NotMapped)
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
#[repr(transparent)]
pub struct Entries([Entry; 512]);

impl Default for Entries {
    #[inline]
    fn default() -> Self {
        Self::EMPTY
    }
}

impl Entries {
    pub const EMPTY: Self = Self([Entry(0); 512]);

    #[must_use]
    #[inline]
    pub const fn new() -> Self {
        Self::EMPTY
    }

    #[inline]
    pub fn iter_entries(&self) -> core::slice::Iter<'_, Entry> {
        self.0.iter()
    }

    #[inline]
    pub fn iter_entries_mut(&mut self) -> core::slice::IterMut<'_, Entry> {
        self.0.iter_mut()
    }

    #[inline]
    pub fn clear(&mut self) {
        self.0.fill(Entry::EMPTY);
    }
}

impl Index<usize> for Entries {
    type Output = Entry;

    #[inline]
    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}
impl IndexMut<usize> for Entries {
    #[inline]
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.0[index]
    }
}
impl Index<u16> for Entries {
    type Output = Entry;

    #[inline]
    fn index(&self, index: u16) -> &Self::Output {
        &self.0[usize::from(index)]
    }
}
impl IndexMut<u16> for Entries {
    #[inline]
    fn index_mut(&mut self, index: u16) -> &mut Self::Output {
        &mut self.0[usize::from(index)]
    }
}

pub struct PageTable<'t> {
    entries: &'t mut Entries,
}

impl<'t> PageTable<'t> {
    #[must_use]
    #[inline]
    pub const fn new(entries: &'t mut Entries) -> Self {
        Self { entries }
    }

    #[must_use]
    #[inline]
    pub const fn entries(&self) -> &Entries {
        self.entries
    }

    #[must_use]
    #[inline]
    pub const fn entries_mut(&mut self) -> &mut Entries {
        self.entries
    }

    fn next_or_create<'a, S: MemSize, A: FrameAllocator<M4KiB>>(
        entry: &'a mut Entry,
        insert_flags: Flags,
        allocator: &mut A,
    ) -> Result<&'a mut Entries, MappingError<S>> {
        if insert_flags.contains(Flags::HUGE_PAGE) {
            return Err(MappingError::UnexpectedLargePage);
        }

        if entry.is_present() {
            // If entry exists, ensure parent flags are at least as permissive
            // We need to add any missing flags (especially USER_ACCESSIBLE)
            entry.add_flags(insert_flags);
            return entry.next_mut();
        }

        // Allocate new frame for the next table level
        let frame = allocator
            .allocate_frame()
            .ok_or(MappingError::FrameAllocationFailed)?;

        entry.set(frame.start_address(), insert_flags | Flags::PRESENT);

        // Get the newly created table and zero it
        let entries = entry.next_mut()?;
        entries.clear();

        Ok(entries)
    }
}

impl Index<usize> for PageTable<'_> {
    type Output = Entry;

    #[inline]
    fn index(&self, index: usize) -> &Self::Output {
        &self.entries[index]
    }
}

impl IndexMut<usize> for PageTable<'_> {
    #[inline]
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.entries[index]
    }
}

impl Mapper<M4KiB, Flags> for PageTable<'_> {
    fn map<A: FrameAllocator<M4KiB>>(
        &mut self,
        page: Page<M4KiB>,
        frame: Frame<M4KiB>,
        flags: Flags,
        fralloc: &mut A,
    ) -> Result<impl beskar_core::arch::paging::CacheFlush<M4KiB>, MappingError<M4KiB>> {
        let parent_flags = if flags.contains(Flags::USER_ACCESSIBLE) {
            Flags::PARENT | Flags::USER_ACCESSIBLE
        } else {
            Flags::PARENT
        };

        let p4_entry = &mut self[usize::from(page.p4_index())];
        let p3 = Self::next_or_create(p4_entry, parent_flags, fralloc)?;
        let p3_entry = &mut p3[usize::from(page.p3_index())];
        let p2 = Self::next_or_create(p3_entry, parent_flags, fralloc)?;
        let p2_entry = &mut p2[usize::from(page.p2_index())];
        let p1 = Self::next_or_create(p2_entry, parent_flags, fralloc)?;
        let p1_entry = &mut p1[usize::from(page.p1_index())];

        if !p1_entry.is_null() {
            return Err(MappingError::AlreadyMapped(Frame::containing_address(
                p1_entry.addr(),
            )));
        }

        p1_entry.set(
            frame.start_address(),
            flags.union(Flags::PRESENT).without(Flags::HUGE_PAGE),
        );

        Ok(super::TlbFlush::new(page))
    }

    fn unmap(
        &mut self,
        page: Page<M4KiB>,
    ) -> Result<
        (
            Frame<M4KiB>,
            impl beskar_core::arch::paging::CacheFlush<M4KiB>,
        ),
        MappingError<M4KiB>,
    > {
        let p4_entry = &mut self[usize::from(page.p4_index())];
        let p3 = p4_entry.next_mut()?;
        let p3_entry = &mut p3[usize::from(page.p3_index())];
        let p2 = p3_entry.next_mut()?;
        let p2_entry = &mut p2[usize::from(page.p2_index())];
        let p1 = p2_entry.next_mut()?;
        let p1_entry = &mut p1[usize::from(page.p1_index())];

        let frame =
            Frame::containing_address(p1_entry.present_addr().ok_or(MappingError::NotMapped)?);

        p1_entry.set(PhysAddr::ZERO, Flags::EMPTY);

        Ok((frame, super::TlbFlush::new(page)))
    }

    fn update_flags(
        &mut self,
        page: Page<M4KiB>,
        flags: Flags,
    ) -> Result<impl beskar_core::arch::paging::CacheFlush<M4KiB>, MappingError<M4KiB>> {
        let p4_entry = &mut self[usize::from(page.p4_index())];
        let p3 = p4_entry.next_mut()?;
        let p3_entry = &mut p3[usize::from(page.p3_index())];
        let p2 = p3_entry.next_mut()?;
        let p2_entry = &mut p2[usize::from(page.p2_index())];
        let p1 = p2_entry.next_mut()?;
        let p1_entry = &mut p1[usize::from(page.p1_index())];

        if !p1_entry.is_present() {
            return Err(MappingError::NotMapped);
        }

        p1_entry.set_flags(flags);

        Ok(super::TlbFlush::new(page))
    }

    fn translate(&self, page: Page<M4KiB>) -> Option<(Frame<M4KiB>, Flags)> {
        let p4_entry = &self[usize::from(page.p4_index())];
        let p3 = p4_entry.next::<M4KiB>().ok()?;
        let p3_entry = &p3[usize::from(page.p3_index())];
        let p2 = p3_entry.next::<M4KiB>().ok()?;
        let p2_entry = &p2[usize::from(page.p2_index())];
        let p1 = p2_entry.next::<M4KiB>().ok()?;
        let p1_entry = &p1[usize::from(page.p1_index())];

        p1_entry
            .is_present()
            .then(|| (Frame::containing_address(p1_entry.addr()), p1_entry.flags()))
    }
}

impl Mapper<M2MiB, Flags> for PageTable<'_> {
    fn map<A: FrameAllocator<M4KiB>>(
        &mut self,
        page: Page<M2MiB>,
        frame: Frame<M2MiB>,
        flags: Flags,
        fralloc: &mut A,
    ) -> Result<impl beskar_core::arch::paging::CacheFlush<M2MiB>, MappingError<M2MiB>> {
        let parent_flags = if flags.contains(Flags::USER_ACCESSIBLE) {
            Flags::PARENT | Flags::USER_ACCESSIBLE
        } else {
            Flags::PARENT
        };

        let p4_entry = &mut self[usize::from(page.p4_index())];
        let p3 = Self::next_or_create(p4_entry, parent_flags, fralloc)?;
        let p3_entry = &mut p3[usize::from(page.p3_index())];
        let p2 = Self::next_or_create(p3_entry, parent_flags, fralloc)?;
        let p2_entry = &mut p2[usize::from(page.p2_index())];

        if !p2_entry.is_null() {
            return Err(MappingError::AlreadyMapped(Frame::containing_address(
                p2_entry.addr(),
            )));
        }

        p2_entry.set(
            frame.start_address(),
            flags.union(Flags::PRESENT).union(Flags::HUGE_PAGE),
        );

        Ok(super::TlbFlush::new(page))
    }

    fn unmap(
        &mut self,
        page: Page<M2MiB>,
    ) -> Result<
        (
            Frame<M2MiB>,
            impl beskar_core::arch::paging::CacheFlush<M2MiB>,
        ),
        MappingError<M2MiB>,
    > {
        let p4_entry = &mut self[usize::from(page.p4_index())];
        let p3 = p4_entry.next_mut()?;
        let p3_entry = &mut p3[usize::from(page.p3_index())];
        let p2 = p3_entry.next_mut()?;
        let p2_entry = &mut p2[usize::from(page.p2_index())];

        let frame =
            Frame::containing_address(p2_entry.present_addr().ok_or(MappingError::NotMapped)?);

        if !p2_entry.flags().contains(Flags::HUGE_PAGE) {
            return Err(MappingError::UnexpectedNotLargePage);
        }

        p2_entry.set(PhysAddr::ZERO, Flags::EMPTY);

        Ok((frame, super::TlbFlush::new(page)))
    }

    fn update_flags(
        &mut self,
        page: Page<M2MiB>,
        flags: Flags,
    ) -> Result<impl beskar_core::arch::paging::CacheFlush<M2MiB>, MappingError<M2MiB>> {
        let p4_entry = &mut self[usize::from(page.p4_index())];
        let p3 = p4_entry.next_mut()?;
        let p3_entry = &mut p3[usize::from(page.p3_index())];
        let p2 = p3_entry.next_mut()?;
        let p2_entry = &mut p2[usize::from(page.p2_index())];

        if !p2_entry.is_present() {
            return Err(MappingError::NotMapped);
        }
        if !p2_entry.is_large() {
            return Err(MappingError::UnexpectedNotLargePage);
        }

        p2_entry.set_flags(flags);

        Ok(super::TlbFlush::new(page))
    }

    fn translate(&self, page: Page<M2MiB>) -> Option<(Frame<M2MiB>, Flags)> {
        let p4_entry = &self[usize::from(page.p4_index())];
        let p3 = p4_entry.next::<M4KiB>().ok()?;
        let p3_entry = &p3[usize::from(page.p3_index())];
        let p2 = p3_entry.next::<M4KiB>().ok()?;
        let p2_entry = &p2[usize::from(page.p2_index())];

        p2_entry
            .is_present()
            .then(|| (Frame::containing_address(p2_entry.addr()), p2_entry.flags()))
    }
}

impl Mapper<M1GiB, Flags> for PageTable<'_> {
    fn map<A: FrameAllocator<M4KiB>>(
        &mut self,
        page: Page<M1GiB>,
        frame: Frame<M1GiB>,
        flags: Flags,
        fralloc: &mut A,
    ) -> Result<impl beskar_core::arch::paging::CacheFlush<M1GiB>, MappingError<M1GiB>> {
        let parent_flags = if flags.contains(Flags::USER_ACCESSIBLE) {
            Flags::PARENT | Flags::USER_ACCESSIBLE
        } else {
            Flags::PARENT
        };

        let p4_entry = &mut self[usize::from(page.p4_index())];
        let p3 = Self::next_or_create(p4_entry, parent_flags, fralloc)?;
        let p3_entry = &mut p3[usize::from(page.p3_index())];

        if !p3_entry.is_null() {
            return Err(MappingError::AlreadyMapped(Frame::containing_address(
                p3_entry.addr(),
            )));
        }

        p3_entry.set(
            frame.start_address(),
            flags.union(Flags::PRESENT).union(Flags::HUGE_PAGE),
        );

        Ok(super::TlbFlush::new(page))
    }

    fn unmap(
        &mut self,
        page: Page<M1GiB>,
    ) -> Result<
        (
            Frame<M1GiB>,
            impl beskar_core::arch::paging::CacheFlush<M1GiB>,
        ),
        MappingError<M1GiB>,
    > {
        let p4_entry = &mut self[usize::from(page.p4_index())];
        let p3 = p4_entry.next_mut()?;
        let p3_entry = &mut p3[usize::from(page.p3_index())];

        let frame =
            Frame::containing_address(p3_entry.present_addr().ok_or(MappingError::NotMapped)?);

        if !p3_entry.flags().contains(Flags::HUGE_PAGE) {
            return Err(MappingError::UnexpectedNotLargePage);
        }

        p3_entry.set(PhysAddr::ZERO, Flags::EMPTY);

        Ok((frame, super::TlbFlush::new(page)))
    }

    fn update_flags(
        &mut self,
        page: Page<M1GiB>,
        flags: Flags,
    ) -> Result<impl beskar_core::arch::paging::CacheFlush<M1GiB>, MappingError<M1GiB>> {
        let p4_entry = &mut self[usize::from(page.p4_index())];
        let p3 = p4_entry.next_mut()?;
        let p3_entry = &mut p3[usize::from(page.p3_index())];

        if !p3_entry.is_present() {
            return Err(MappingError::NotMapped);
        }
        if !p3_entry.is_large() {
            return Err(MappingError::UnexpectedNotLargePage);
        }

        p3_entry.set_flags(flags);

        Ok(super::TlbFlush::new(page))
    }

    fn translate(&self, page: Page<M1GiB>) -> Option<(Frame<M1GiB>, Flags)> {
        let p4_entry = &self[usize::from(page.p4_index())];
        let p3 = p4_entry.next::<M4KiB>().ok()?;
        let p3_entry = &p3[usize::from(page.p3_index())];

        p3_entry
            .is_present()
            .then(|| (Frame::containing_address(p3_entry.addr()), p3_entry.flags()))
    }
}

impl Translator<Flags> for PageTable<'_> {
    fn translate_addr(&self, addr: VirtAddr) -> Option<(PhysAddr, Flags)> {
        // Here, we need to be careful, as the address can be in any size
        // of page. We need to check for it in every level of the page table.
        let p4_entry = &self[usize::from(addr.p4_index())];
        let p3 = p4_entry.next::<M4KiB>().ok()?;
        let p3_entry = &p3[usize::from(addr.p3_index())];
        if p3_entry.is_present() && p3_entry.is_large() {
            return Some((
                PhysAddr::new_truncate(p3_entry.addr().as_u64() + addr.as_u64() % M1GiB::SIZE),
                p3_entry.flags(),
            ));
        }
        let p2 = p3_entry.next::<M4KiB>().ok()?;
        let p2_entry = &p2[usize::from(addr.p2_index())];
        if p2_entry.is_present() && p2_entry.is_large() {
            return Some((
                PhysAddr::new_truncate(p2_entry.addr().as_u64() + addr.as_u64() % M2MiB::SIZE),
                p2_entry.flags(),
            ));
        }
        let p1 = p2_entry.next::<M4KiB>().ok()?;
        let p1_entry = &p1[usize::from(addr.p1_index())];
        if !p1_entry.is_present() {
            return None;
        }

        Some((
            PhysAddr::new_truncate(p1_entry.addr().as_u64() + addr.as_u64() % M4KiB::SIZE),
            p1_entry.flags(),
        ))
    }
}

pub struct OffsetPageTable<'t> {
    entries: &'t mut Entries,
    offset: VirtAddr,
}

impl<'t> OffsetPageTable<'t> {
    #[must_use]
    #[inline]
    pub const fn new(entries: &'t mut Entries, offset: VirtAddr) -> Self {
        Self { entries, offset }
    }

    #[must_use]
    #[inline]
    pub const fn entries(&self) -> &Entries {
        self.entries
    }

    #[must_use]
    #[inline]
    pub const fn entries_mut(&mut self) -> &mut Entries {
        self.entries
    }

    #[must_use]
    /// Finds the next table in the page table hierarchy.
    ///
    /// As this function isn't aware of the page size, it doesn't check for huge pages.
    fn next_table(offset: VirtAddr, entry: &Entry) -> Option<&Entries> {
        if !entry.is_present() || entry.is_large() {
            return None;
        }
        let pt_vaddr = offset + entry.addr().as_u64();
        let pt_ptr = pt_vaddr.as_ptr::<Entries>();
        Some(unsafe { &*pt_ptr })
    }

    #[must_use]
    #[expect(clippy::needless_pass_by_ref_mut, reason = "False positive")]
    /// Finds the next table in the page table hierarchy.
    ///
    /// As this function isn't aware of the page size, it doesn't check for huge pages.
    fn next_table_mut(offset: VirtAddr, entry: &mut Entry) -> Option<&mut Entries> {
        if !entry.is_present() || entry.is_large() {
            return None;
        }
        let pt_vaddr = offset + entry.addr().as_u64();
        let pt_ptr = pt_vaddr.as_mut_ptr::<Entries>();
        Some(unsafe { &mut *pt_ptr })
    }

    fn next_or_create<'a, S: MemSize, A: FrameAllocator<M4KiB>>(
        offset: VirtAddr,
        entry: &'a mut Entry,
        insert_flags: Flags,
        allocator: &mut A,
    ) -> Result<&'a mut Entries, MappingError<S>> {
        if insert_flags.contains(Flags::HUGE_PAGE) {
            return Err(MappingError::UnexpectedLargePage);
        }

        if entry.is_present() {
            // If entry exists, ensure parent flags are at least as permissive
            entry.add_flags(insert_flags);
            return Self::next_table_mut(offset, entry).ok_or(MappingError::NotMapped);
        }

        // Allocate new frame for the next table level
        let frame = allocator
            .allocate_frame()
            .ok_or(MappingError::FrameAllocationFailed)?;

        entry.set(frame.start_address(), insert_flags | Flags::PRESENT);

        // Get the newly created table and zero it
        let next_table = Self::next_table_mut(offset, entry).ok_or(MappingError::NotMapped)?;
        next_table.clear();

        Ok(next_table)
    }
}

impl Mapper<M4KiB, Flags> for OffsetPageTable<'_> {
    fn map<A: FrameAllocator<M4KiB>>(
        &mut self,
        page: Page<M4KiB>,
        frame: Frame<M4KiB>,
        flags: Flags,
        allocator: &mut A,
    ) -> Result<impl beskar_core::arch::paging::CacheFlush<M4KiB>, MappingError<M4KiB>> {
        let parent_flags = if flags.contains(Flags::USER_ACCESSIBLE) {
            Flags::PARENT | Flags::USER_ACCESSIBLE
        } else {
            Flags::PARENT
        };

        let p4_entry = &mut self.entries[usize::from(page.p4_index())];
        let p3 = Self::next_or_create(self.offset, p4_entry, parent_flags, allocator)?;
        let p3_entry = &mut p3[usize::from(page.p3_index())];
        let p2 = Self::next_or_create(self.offset, p3_entry, parent_flags, allocator)?;
        let p2_entry = &mut p2[usize::from(page.p2_index())];
        let p1 = Self::next_or_create(self.offset, p2_entry, parent_flags, allocator)?;
        let p1_entry = &mut p1[usize::from(page.p1_index())];

        if !p1_entry.is_null() {
            return Err(MappingError::AlreadyMapped(Frame::containing_address(
                p1_entry.addr(),
            )));
        }

        p1_entry.set(
            frame.start_address(),
            flags.union(Flags::PRESENT).without(Flags::HUGE_PAGE),
        );

        Ok(super::TlbFlush::new(page))
    }

    fn translate(&self, page: Page<M4KiB>) -> Option<(Frame<M4KiB>, Flags)> {
        let p4_entry = &self.entries()[usize::from(page.p4_index())];
        let p3 = Self::next_table(self.offset, p4_entry)?;
        let p3_entry = &p3[usize::from(page.p3_index())];
        let p2 = Self::next_table(self.offset, p3_entry)?;
        let p2_entry = &p2[usize::from(page.p2_index())];
        let p1 = Self::next_table(self.offset, p2_entry)?;
        let p1_entry = &p1[usize::from(page.p1_index())];

        p1_entry
            .is_present()
            .then(|| (Frame::containing_address(p1_entry.addr()), p1_entry.flags()))
    }

    fn unmap(
        &mut self,
        page: Page<M4KiB>,
    ) -> Result<
        (
            Frame<M4KiB>,
            impl beskar_core::arch::paging::CacheFlush<M4KiB>,
        ),
        MappingError<M4KiB>,
    > {
        let p4_entry = &mut self.entries[usize::from(page.p4_index())];
        let p3 = Self::next_table_mut(self.offset, p4_entry).ok_or(MappingError::NotMapped)?;
        let p3_entry = &mut p3[usize::from(page.p3_index())];
        let p2 = Self::next_table_mut(self.offset, p3_entry).ok_or(MappingError::NotMapped)?;
        let p2_entry = &mut p2[usize::from(page.p2_index())];
        let p1 = Self::next_table_mut(self.offset, p2_entry).ok_or(MappingError::NotMapped)?;
        let p1_entry = &mut p1[usize::from(page.p1_index())];

        let frame =
            Frame::containing_address(p1_entry.present_addr().ok_or(MappingError::NotMapped)?);

        p1_entry.set(PhysAddr::ZERO, Flags::EMPTY);

        Ok((frame, super::TlbFlush::new(page)))
    }

    fn update_flags(
        &mut self,
        page: Page<M4KiB>,
        flags: Flags,
    ) -> Result<impl beskar_core::arch::paging::CacheFlush<M4KiB>, MappingError<M4KiB>> {
        let p4_entry = &mut self.entries[usize::from(page.p4_index())];
        let p3 = Self::next_table_mut(self.offset, p4_entry).ok_or(MappingError::NotMapped)?;
        let p3_entry = &mut p3[usize::from(page.p3_index())];
        let p2 = Self::next_table_mut(self.offset, p3_entry).ok_or(MappingError::NotMapped)?;
        let p2_entry = &mut p2[usize::from(page.p2_index())];
        let p1 = Self::next_table_mut(self.offset, p2_entry).ok_or(MappingError::NotMapped)?;
        let p1_entry = &mut p1[usize::from(page.p1_index())];

        if !p1_entry.is_present() {
            return Err(MappingError::NotMapped);
        }

        p1_entry.set_flags(flags);

        Ok(super::TlbFlush::new(page))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use beskar_core::arch::PhysAddr;

    #[test]
    fn test_flags_operations() {
        let flags = Flags::PRESENT | Flags::WRITABLE;
        assert!(flags.contains(Flags::PRESENT));
        assert!(flags.contains(Flags::WRITABLE));
        assert!(!flags.contains(Flags::USER_ACCESSIBLE));

        let new_flags = flags.union(Flags::USER_ACCESSIBLE);
        assert!(new_flags.contains(Flags::USER_ACCESSIBLE));

        let intersection = flags.intersection(Flags::WRITABLE | Flags::USER_ACCESSIBLE);
        assert!(intersection.contains(Flags::WRITABLE));
        assert!(!intersection.contains(Flags::USER_ACCESSIBLE));

        let without_flags = flags.without(Flags::WRITABLE);
        assert!(!without_flags.contains(Flags::WRITABLE));
    }

    #[test]
    fn test_entry_operations() {
        let mut entry = Entry::default();
        let addr = PhysAddr::new_truncate(0x2000);
        let flags = Flags::PRESENT | Flags::WRITABLE;

        entry.set(addr, flags);
        assert_eq!(entry.addr(), addr);
        assert!(entry.flags().contains(Flags::PRESENT));
        assert!(entry.flags().contains(Flags::WRITABLE));

        entry.add_flags(Flags::USER_ACCESSIBLE);
        assert!(entry.flags().contains(Flags::USER_ACCESSIBLE));
        assert!(entry.flags().contains(Flags::PRESENT));
        assert!(entry.flags().contains(Flags::WRITABLE));

        // Test set_flags replaces flags
        entry.set_flags(Flags::PRESENT | Flags::NO_EXECUTE);
        assert!(entry.flags().contains(Flags::PRESENT));
        assert!(entry.flags().contains(Flags::NO_EXECUTE));
        assert!(!entry.flags().contains(Flags::WRITABLE));
        assert!(!entry.flags().contains(Flags::USER_ACCESSIBLE));
    }

    #[test]
    fn test_entries_clear() {
        let mut entries = Entries::default();
        entries[0_usize].set(PhysAddr::new_truncate(0x1000), Flags::PRESENT);
        entries[1_usize].set(PhysAddr::new_truncate(0x2000), Flags::WRITABLE);

        entries.clear();
        for entry in entries.iter_entries() {
            assert!(entry.is_null());
        }
    }

    #[test]
    fn test_flags_bitor_assign() {
        let mut flags = Flags::PRESENT;
        flags |= Flags::WRITABLE;
        assert!(flags.contains(Flags::PRESENT));
        assert!(flags.contains(Flags::WRITABLE));
    }

    #[test]
    fn test_flags_bitand_assign() {
        let mut flags = Flags::PRESENT | Flags::WRITABLE | Flags::USER_ACCESSIBLE;
        flags &= Flags::PRESENT | Flags::WRITABLE;
        assert!(flags.contains(Flags::PRESENT));
        assert!(flags.contains(Flags::WRITABLE));
        assert!(!flags.contains(Flags::USER_ACCESSIBLE));
    }

    #[test]
    fn test_entry_set_flags_replaces() {
        let mut entry = Entry::default();
        let addr = PhysAddr::new_truncate(0x1000);

        entry.set(
            addr,
            Flags::PRESENT | Flags::WRITABLE | Flags::USER_ACCESSIBLE,
        );
        assert!(entry.flags().contains(Flags::WRITABLE));
        assert!(entry.flags().contains(Flags::USER_ACCESSIBLE));

        // set_flags should REPLACE flags, not OR them
        entry.set_flags(Flags::PRESENT | Flags::NO_EXECUTE);
        assert!(entry.flags().contains(Flags::PRESENT));
        assert!(entry.flags().contains(Flags::NO_EXECUTE));
        assert!(!entry.flags().contains(Flags::WRITABLE));
        assert!(!entry.flags().contains(Flags::USER_ACCESSIBLE));

        // Verify address is preserved
        assert_eq!(entry.addr(), addr);
    }

    #[test]
    fn test_entry_add_flags_accumulates() {
        let mut entry = Entry::default();
        let addr = PhysAddr::new_truncate(0x2000);

        entry.set(addr, Flags::PRESENT);
        entry.add_flags(Flags::WRITABLE);
        assert!(entry.flags().contains(Flags::PRESENT));
        assert!(entry.flags().contains(Flags::WRITABLE));

        entry.add_flags(Flags::USER_ACCESSIBLE);
        assert!(entry.flags().contains(Flags::PRESENT));
        assert!(entry.flags().contains(Flags::WRITABLE));
        assert!(entry.flags().contains(Flags::USER_ACCESSIBLE));

        // Verify address is preserved
        assert_eq!(entry.addr(), addr);
    }

    #[test]
    fn test_entry_helper_methods() {
        let mut entry = Entry::default();
        entry.set(
            PhysAddr::new_truncate(0x1000),
            Flags::PRESENT | Flags::USER_ACCESSIBLE | Flags::WRITABLE,
        );

        assert!(entry.is_present());
        assert!(entry.is_user_accessible());
        assert!(entry.is_writable());
        assert!(!entry.is_large());

        entry.set_flags(Flags::PRESENT | Flags::HUGE_PAGE);
        assert!(entry.is_large());
        assert!(!entry.is_writable());
    }

    // TODO: How to test page tables?
}
