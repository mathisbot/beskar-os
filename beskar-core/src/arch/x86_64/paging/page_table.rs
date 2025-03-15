//! Page table module.
//!
//! This only supports recursive page tables, as it is the only type of page table
//! that is used in the kernel (for now at least).

use core::ops::{Index, IndexMut};

use crate::arch::commons::{
    PhysAddr, VirtAddr,
    paging::{Frame, FrameAllocator, M1GiB, M2MiB, M4KiB, Mapper, MemSize, Page, Translator},
};

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

    const ALL: Self = Self(0x8000_0000_0000_01FF);
    pub const EMPTY: Self = Self(0);
    /// A set of flags that are used to mark the parent entries in the page table.
    /// The flags are present and writable.
    ///
    /// ## Warning
    ///
    /// If any child page is USER ACCESSIBLE, then the parent page must also be USER ACCESSIBLE.
    const PARENT: Self = Self(1 | (1 << 1));

    #[must_use]
    #[inline]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    #[must_use]
    #[inline]
    pub fn contains(self, other: Self) -> bool {
        (self & other) == other
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

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct Entry(u64);

impl Entry {
    #[must_use]
    #[inline]
    pub const fn as_u64(self) -> u64 {
        self.0
    }

    #[must_use]
    #[inline]
    pub const fn flags(self) -> Flags {
        Flags(self.0 & Flags::ALL.0)
    }

    #[must_use]
    #[inline]
    pub const fn addr(self) -> PhysAddr {
        PhysAddr::new(self.0 & 0x000f_ffff_ffff_f000)
    }

    #[must_use]
    #[inline]
    pub fn frame_start(self) -> Option<PhysAddr> {
        assert!(!self.flags().contains(Flags::HUGE_PAGE), "Huge page");

        if self.flags().contains(Flags::PRESENT) {
            Some(self.addr())
        } else {
            None
        }
    }

    #[inline]
    pub const fn set(&mut self, addr: PhysAddr, flags: Flags) {
        self.0 = addr.as_u64() | flags.0;
    }

    #[inline]
    /// ORs the flags with the current flags.
    pub const fn update_flags(&mut self, flags: Flags) {
        self.0 |= flags.0;
    }

    #[must_use]
    #[inline]
    pub const fn is_null(self) -> bool {
        self.0 == 0
    }
}

#[derive(Debug, PartialEq, Eq)]
#[repr(transparent)]
pub struct Entries([Entry; 512]);

impl Default for Entries {
    #[inline]
    fn default() -> Self {
        Self([Entry(0); 512])
    }
}

impl Entries {
    #[must_use]
    #[inline]
    pub const fn new() -> Self {
        Self([Entry(0); 512])
    }

    #[inline]
    pub fn iter_entries(&self) -> core::slice::Iter<Entry> {
        self.0.iter()
    }

    #[inline]
    pub fn iter_entries_mut(&mut self) -> core::slice::IterMut<Entry> {
        self.0.iter_mut()
    }

    #[inline]
    pub fn clear(&mut self) {
        self.0.iter_mut().for_each(|entry| *entry = Entry(0));
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

pub struct PageTable<'t> {
    entries: &'t mut Entries,
    recursive_index: u16,
}

impl<'t> PageTable<'t> {
    #[must_use]
    #[inline]
    pub fn new(entries: &'t mut Entries) -> Self {
        let page = Page::<M4KiB>::containing_address(VirtAddr::new(entries.0.as_ptr() as u64));
        let l4_index = page.p4_index();

        if page.p3_index() != l4_index || page.p2_index() != l4_index || page.p1_index() != l4_index
        {
            unimplemented!("Non-recursive page table");
        }

        Self {
            entries,
            recursive_index: l4_index,
        }
    }

    #[must_use]
    #[inline]
    pub const fn new_from_index(entries: &'t mut Entries, recursive_index: u16) -> Self {
        Self {
            entries,
            recursive_index,
        }
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

    unsafe fn create_next_level<'a, A: FrameAllocator<M4KiB>>(
        entry: &'a mut Entry,
        next_table: Page,
        insert_flags: Flags,
        allocator: &mut A,
    ) -> &'a mut Entries {
        let mut existed = true;

        if entry.is_null() {
            existed = false;

            let frame = allocator.allocate_frame().unwrap();
            entry.set(
                frame.start_address(),
                Flags::PRESENT | Flags::WRITABLE | insert_flags,
            );
        } else {
            entry.update_flags(insert_flags);
        }

        assert_eq!(
            entry.flags() & Flags::HUGE_PAGE,
            Flags::EMPTY,
            "Cannot create huge page"
        );

        let entries_ptr = next_table.start_address().as_mut_ptr::<Entries>();
        let entries = unsafe { &mut *entries_ptr };
        if !existed {
            entries.clear();
        }

        entries
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

#[must_use]
fn get_p3<S: MemSize>(page: Page<S>, recursive_index: u16) -> Page {
    let ri = u64::from(recursive_index);
    let p4_idx = u64::from(page.p4_index());
    let vaddr = VirtAddr::new_extend((ri << 39) | (ri << 30) | (ri << 21) | (p4_idx << 12));
    Page::containing_address(vaddr)
}

#[must_use]
fn get_p2_2mib(page: Page<M2MiB>, recursive_index: u16) -> Page {
    let ri = u64::from(recursive_index);
    let p4_idx = u64::from(page.p4_index());
    let p3_idx = u64::from(page.p3_index());
    let vaddr = VirtAddr::new_extend((ri << 39) | (ri << 30) | (p4_idx << 21) | (p3_idx << 12));
    Page::containing_address(vaddr)
}
#[must_use]
fn get_p2_4kib(page: Page<M4KiB>, recursive_index: u16) -> Page {
    let ri = u64::from(recursive_index);
    let p4_idx = u64::from(page.p4_index());
    let p3_idx = u64::from(page.p3_index());
    let vaddr = VirtAddr::new_extend((ri << 39) | (ri << 30) | (p4_idx << 21) | (p3_idx << 12));
    Page::containing_address(vaddr)
}

#[must_use]
fn get_p1(page: Page<M4KiB>, recursive_index: u16) -> Page {
    let ri = u64::from(recursive_index);
    let p4_idx = u64::from(page.p4_index());
    let p3_idx = u64::from(page.p3_index());
    let p2_idx = u64::from(page.p2_index());
    let vaddr = VirtAddr::new_extend((ri << 39) | (p4_idx << 30) | (p3_idx << 21) | (p2_idx << 12));
    Page::containing_address(vaddr)
}

impl Mapper<M4KiB> for PageTable<'_> {
    fn map<A: FrameAllocator<M4KiB>>(
        &mut self,
        page: Page<M4KiB>,
        frame: Frame<M4KiB>,
        flags: Flags,
        fralloc: &mut A,
    ) -> impl crate::arch::commons::paging::CacheFlush<M4KiB> {
        let ri = self.recursive_index;
        let parent_flags = if flags.contains(Flags::USER_ACCESSIBLE) {
            Flags::PARENT | Flags::USER_ACCESSIBLE
        } else {
            Flags::PARENT
        };

        let p3_page = get_p3(page, ri);
        let p3 = unsafe {
            Self::create_next_level(
                &mut self[usize::from(page.p4_index())],
                p3_page,
                parent_flags,
                fralloc,
            )
        };

        let p2_page = get_p2_4kib(page, ri);
        let p2 = unsafe {
            Self::create_next_level(
                &mut p3[usize::from(page.p3_index())],
                p2_page,
                parent_flags,
                fralloc,
            )
        };

        let p1_page = get_p1(page, ri);
        let p1 = unsafe {
            Self::create_next_level(
                &mut p2[usize::from(page.p2_index())],
                p1_page,
                parent_flags,
                fralloc,
            )
        };

        assert!(
            p1[usize::from(page.p1_index())].is_null(),
            "Page already mapped"
        );
        p1[usize::from(page.p1_index())].set(frame.start_address(), flags | Flags::PRESENT);

        super::TlbFlush::new(page)
    }

    fn unmap(
        &mut self,
        page: Page<M4KiB>,
    ) -> Option<(
        Frame<M4KiB>,
        impl crate::arch::commons::paging::CacheFlush<M4KiB>,
    )> {
        let p4_entry = &self[usize::from(page.p4_index())];
        p4_entry.frame_start()?;

        let p3 = unsafe {
            &*get_p3(page, self.recursive_index)
                .start_address()
                .as_mut_ptr::<Entries>()
        };
        let p3_entry = &p3[usize::from(page.p3_index())];
        p3_entry.frame_start()?;

        let p2 = unsafe {
            &*get_p2_4kib(page, self.recursive_index)
                .start_address()
                .as_mut_ptr::<Entries>()
        };
        let p2_entry = &p2[usize::from(page.p2_index())];
        p2_entry.frame_start()?;

        let p1 = unsafe {
            &mut *get_p1(page, self.recursive_index)
                .start_address()
                .as_mut_ptr::<Entries>()
        };
        let p1_entry = &mut p1[usize::from(page.p1_index())];
        let frame = Frame::from_start_address(p1_entry.frame_start()?).unwrap();

        p1_entry.set(PhysAddr::new(0), Flags::EMPTY);

        Some((frame, super::TlbFlush::new(page)))
    }

    fn update_flags(
        &mut self,
        page: Page<M4KiB>,
        flags: Flags,
    ) -> Option<impl crate::arch::commons::paging::CacheFlush<M4KiB>> {
        let p4_entry = &self[usize::from(page.p4_index())];
        if p4_entry.is_null() {
            return None;
        }

        let p3 = unsafe {
            &mut *get_p3(page, self.recursive_index)
                .start_address()
                .as_mut_ptr::<Entries>()
        };
        let p3_entry = &mut p3[usize::from(page.p3_index())];
        if p3_entry.is_null() {
            return None;
        }

        let p2 = unsafe {
            &mut *get_p2_4kib(page, self.recursive_index)
                .start_address()
                .as_mut_ptr::<Entries>()
        };
        let p2_entry = &mut p2[usize::from(page.p2_index())];
        if p2_entry.is_null() {
            return None;
        }

        let p1 = unsafe {
            &mut *get_p1(page, self.recursive_index)
                .start_address()
                .as_mut_ptr::<Entries>()
        };
        let p1_entry = &mut p1[usize::from(page.p1_index())];
        if p1_entry.is_null() {
            return None;
        }

        let addr = p1_entry.addr();
        p1_entry.set(addr, flags);

        Some(super::TlbFlush::new(page))
    }

    fn translate(&self, page: Page<M4KiB>) -> Option<(Frame<M4KiB>, Flags)> {
        let p4_entry = &self[usize::from(page.p4_index())];
        if p4_entry.is_null() {
            return None;
        }
        let p3 = unsafe {
            &*get_p3(page, self.recursive_index)
                .start_address()
                .as_ptr::<Entries>()
        };
        let p3_entry = &p3[usize::from(page.p3_index())];
        if p3_entry.is_null() {
            return None;
        }
        let p2 = unsafe {
            &*get_p2_4kib(page, self.recursive_index)
                .start_address()
                .as_ptr::<Entries>()
        };
        let p2_entry = &p2[usize::from(page.p2_index())];
        if p2_entry.is_null() {
            return None;
        }
        let p1 = unsafe {
            &*get_p1(page, self.recursive_index)
                .start_address()
                .as_ptr::<Entries>()
        };
        let p1_entry = &p1[usize::from(page.p1_index())];
        if p1_entry.is_null() {
            None
        } else {
            Some((
                Frame::from_start_address(p1_entry.addr()).unwrap(),
                p1_entry.flags(),
            ))
        }
    }
}

impl Mapper<M2MiB> for PageTable<'_> {
    fn map<A: FrameAllocator<M4KiB>>(
        &mut self,
        page: Page<M2MiB>,
        frame: Frame<M2MiB>,
        flags: Flags,
        fralloc: &mut A,
    ) -> impl crate::arch::commons::paging::CacheFlush<M2MiB> {
        let ri = self.recursive_index;
        let parent_flags = if flags.contains(Flags::USER_ACCESSIBLE) {
            Flags::PARENT | Flags::USER_ACCESSIBLE
        } else {
            Flags::PARENT
        };

        let p3_page = get_p3(page, ri);
        let p3 = unsafe {
            Self::create_next_level(
                &mut self[usize::from(page.p4_index())],
                p3_page,
                parent_flags,
                fralloc,
            )
        };

        let p2_page = get_p2_2mib(page, ri);
        let p2 = unsafe {
            Self::create_next_level(
                &mut p3[usize::from(page.p3_index())],
                p2_page,
                parent_flags,
                fralloc,
            )
        };

        assert!(
            p2[usize::from(page.p2_index())].is_null(),
            "Page already mapped"
        );
        p2[usize::from(page.p2_index())].set(frame.start_address(), Flags::HUGE_PAGE | flags);

        super::TlbFlush::new(page)
    }

    fn unmap(
        &mut self,
        page: Page<M2MiB>,
    ) -> Option<(
        Frame<M2MiB>,
        impl crate::arch::commons::paging::CacheFlush<M2MiB>,
    )> {
        let p4_entry = &self[usize::from(page.p4_index())];
        p4_entry.frame_start()?;

        let p3 = unsafe {
            &mut *get_p3(page, self.recursive_index)
                .start_address()
                .as_mut_ptr::<Entries>()
        };
        let p3_entry = &mut p3[usize::from(page.p3_index())];
        p3_entry.frame_start()?;

        let p2 = unsafe {
            &mut *get_p2_2mib(page, self.recursive_index)
                .start_address()
                .as_mut_ptr::<Entries>()
        };
        let p2_entry = &mut p2[usize::from(page.p2_index())];
        let flags = p2_entry.flags();

        if !flags.contains(Flags::PRESENT) {
            return None;
        }
        assert!(flags.contains(Flags::HUGE_PAGE), "Not a huge page");

        let frame = Frame::from_start_address(p2_entry.addr()).unwrap();

        p2_entry.set(PhysAddr::new(0), Flags::EMPTY);

        Some((frame, super::TlbFlush::new(page)))
    }

    fn update_flags(
        &mut self,
        page: Page<M2MiB>,
        flags: Flags,
    ) -> Option<impl crate::arch::commons::paging::CacheFlush<M2MiB>> {
        let p4_entry = &self[usize::from(page.p4_index())];
        if p4_entry.is_null() {
            return None;
        }

        let p3 = unsafe {
            &mut *get_p3(page, self.recursive_index)
                .start_address()
                .as_mut_ptr::<Entries>()
        };
        let p3_entry = &mut p3[usize::from(page.p3_index())];
        if p3_entry.is_null() {
            return None;
        }

        let p2 = unsafe {
            &mut *get_p2_2mib(page, self.recursive_index)
                .start_address()
                .as_mut_ptr::<Entries>()
        };
        let p2_entry = &mut p2[usize::from(page.p2_index())];
        if p2_entry.is_null() {
            return None;
        }

        let addr = p2_entry.addr();
        p2_entry.set(addr, flags | Flags::HUGE_PAGE);

        Some(super::TlbFlush::new(page))
    }

    fn translate(&self, page: Page<M2MiB>) -> Option<(Frame<M2MiB>, Flags)> {
        let p4_entry = &self[usize::from(page.p4_index())];
        if p4_entry.is_null() {
            return None;
        }
        let p3 = unsafe {
            &*get_p3(page, self.recursive_index)
                .start_address()
                .as_ptr::<Entries>()
        };
        let p3_entry = &p3[usize::from(page.p3_index())];
        if p3_entry.is_null() {
            return None;
        }
        let p2 = unsafe {
            &*get_p2_2mib(page, self.recursive_index)
                .start_address()
                .as_ptr::<Entries>()
        };
        let p2_entry = &p2[usize::from(page.p2_index())];
        if p2_entry.is_null() {
            None
        } else {
            Some((
                Frame::from_start_address(p2_entry.addr()).unwrap(),
                p2_entry.flags(),
            ))
        }
    }
}

impl Mapper<M1GiB> for PageTable<'_> {
    fn map<A: FrameAllocator<M4KiB>>(
        &mut self,
        page: Page<M1GiB>,
        frame: Frame<M1GiB>,
        flags: Flags,
        fralloc: &mut A,
    ) -> impl crate::arch::commons::paging::CacheFlush<M1GiB> {
        let parent_flags = if flags.contains(Flags::USER_ACCESSIBLE) {
            Flags::PARENT | Flags::USER_ACCESSIBLE
        } else {
            Flags::PARENT
        };

        let p3_page = get_p3(page, self.recursive_index);
        let p3 = unsafe {
            Self::create_next_level(
                &mut self[usize::from(page.p4_index())],
                p3_page,
                parent_flags,
                fralloc,
            )
        };

        assert!(
            p3[usize::from(page.p3_index())].is_null(),
            "Page already mapped"
        );
        p3[usize::from(page.p3_index())].set(frame.start_address(), Flags::HUGE_PAGE | flags);

        super::TlbFlush::new(page)
    }

    fn unmap(
        &mut self,
        page: Page<M1GiB>,
    ) -> Option<(
        Frame<M1GiB>,
        impl crate::arch::commons::paging::CacheFlush<M1GiB>,
    )> {
        let p4_entry = &self[usize::from(page.p4_index())];
        p4_entry.frame_start()?;

        let p3 = unsafe {
            &mut *get_p3(page, self.recursive_index)
                .start_address()
                .as_mut_ptr::<Entries>()
        };
        let p3_entry = &mut p3[usize::from(page.p3_index())];
        let flags = p3_entry.flags();

        if !flags.contains(Flags::PRESENT) {
            return None;
        }
        assert!(flags.contains(Flags::HUGE_PAGE), "Not a huge page");

        let frame = Frame::from_start_address(p3_entry.addr()).unwrap();

        p3_entry.set(PhysAddr::new(0), Flags::EMPTY);

        Some((frame, super::TlbFlush::new(page)))
    }

    fn update_flags(
        &mut self,
        page: Page<M1GiB>,
        flags: Flags,
    ) -> Option<impl crate::arch::commons::paging::CacheFlush<M1GiB>> {
        let p4_entry = &self[usize::from(page.p4_index())];
        if p4_entry.is_null() {
            return None;
        }

        let p3 = unsafe {
            &mut *get_p3(page, self.recursive_index)
                .start_address()
                .as_mut_ptr::<Entries>()
        };
        let p3_entry = &mut p3[usize::from(page.p3_index())];
        if p3_entry.is_null() {
            return None;
        }

        let addr = p3_entry.addr();
        p3_entry.set(addr, flags | Flags::HUGE_PAGE);

        Some(super::TlbFlush::new(page))
    }

    fn translate(&self, page: Page<M1GiB>) -> Option<(Frame<M1GiB>, Flags)> {
        let p4_entry = &self[usize::from(page.p4_index())];
        if p4_entry.is_null() {
            return None;
        }
        let p3 = unsafe {
            &*get_p3(page, self.recursive_index)
                .start_address()
                .as_ptr::<Entries>()
        };
        let p3_entry = &p3[usize::from(page.p3_index())];
        if p3_entry.is_null() {
            None
        } else {
            Some((
                Frame::from_start_address(p3_entry.addr()).unwrap(),
                p3_entry.flags(),
            ))
        }
    }
}

impl Translator for PageTable<'_> {
    fn translate_addr(&self, addr: VirtAddr) -> Option<PhysAddr> {
        // Here, we need to be careful, as the address can be in any size
        // of page. We need to check for it in every level of the page table.
        let page = Page::containing_address(addr);

        let p4_entry = &self[usize::from(addr.p4_index())];
        if p4_entry.is_null() {
            return None;
        }
        let p3 = unsafe {
            &*get_p3(page, self.recursive_index)
                .start_address()
                .as_ptr::<Entries>()
        };
        let p3_entry = &p3[usize::from(addr.p3_index())];
        if p3_entry.is_null() {
            return None;
        }
        if p3_entry.flags() & Flags::HUGE_PAGE != Flags::EMPTY {
            return Some(PhysAddr::new(
                p3_entry.addr().as_u64() + addr.as_u64() % M1GiB::SIZE,
            ));
        }

        let p2 = unsafe {
            &*get_p2_4kib(page, self.recursive_index)
                .start_address()
                .as_ptr::<Entries>()
        };

        let p2_entry = &p2[usize::from(addr.p2_index())];
        if p2_entry.is_null() {
            return None;
        }
        if p2_entry.flags() & Flags::HUGE_PAGE != Flags::EMPTY {
            return Some(PhysAddr::new(
                p2_entry.addr().as_u64() + addr.as_u64() % M2MiB::SIZE,
            ));
        }

        let p1 = unsafe {
            &*get_p1(page, self.recursive_index)
                .start_address()
                .as_ptr::<Entries>()
        };
        let p1_entry = &p1[usize::from(addr.p1_index())];
        if p1_entry.is_null() {
            return None;
        }

        Some(PhysAddr::new(
            p1_entry.addr().as_u64() + addr.as_u64() % M4KiB::SIZE,
        ))
    }
}
