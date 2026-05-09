//! Virtual memory manager facade.
//!
//! This module separates process-local mappings (lower half) from kernel-global
//! mappings (upper half).

use super::frame_alloc;
use crate::{arch::cpuid, process};
use beskar_core::arch::{
    VirtAddr,
    paging::{
        CacheFlush as _, Frame, M4KiB, Mapper, MappingError, MemSize, Page, PageRangeInclusive,
    },
};
use beskar_hal::{
    paging::page_table::{Entries, Flags, PageTable},
    registers::Efer,
};
use bootloader_api::KERNEL_POOL_BASE;
use hyperdrive::once::Once;

/// The recursive index used for the recursive page table mapping.
static KERNEL_PT_RECURSIVE_INDEX: Once<u16> = Once::uninit();

fn prewire_shared_kernel_pool_entries(kernel_pt: &mut PageTable<'static>) {
    frame_alloc::with_frame_allocator(|frame_allocator| {
        for entry in kernel_pt
            .entries_mut()
            .iter_entries_mut()
            .skip(KERNEL_POOL_BASE.p4_index() as usize)
            .filter(|e| !e.is_present())
        {
            let p3_frame = frame_allocator
                .alloc::<M4KiB>()
                .expect("Failed to allocate shared kernel P3 table");
            entry.set(
                p3_frame.start_address(),
                Flags::PRESENT | Flags::WRITABLE | Flags::NO_EXECUTE,
            );

            let p3 = entry.next_mut::<M4KiB>().unwrap();
            *p3 = Entries::EMPTY;
        }
    });
}

pub fn init(recursive_index: u16) {
    KERNEL_PT_RECURSIVE_INDEX.call_once(|| recursive_index);

    let mut kernel_pt = {
        let vaddr = VirtAddr::from_pt_indices(
            recursive_index,
            recursive_index,
            recursive_index,
            recursive_index,
            0,
        );
        // Safety: The page table given by the bootloader is valid
        let raw_pt = unsafe { &mut *vaddr.as_mut_ptr::<Entries>() };
        PageTable::new(raw_pt)
    };

    if cpuid::check_feature(cpuid::CpuFeature::TCE) {
        unsafe { Efer::insert_flags(Efer::TRANSLATION_CACHE_EXTENSION) };
    }

    prewire_shared_kernel_pool_entries(&mut kernel_pt);
}

pub mod phys_map {
    pub use crate::mem::phys_map::PhysicalMapping;
}

pub mod kernel {
    use super::*;
    use crate::mem::page_alloc;
    use bootloader_api::KERNEL_AS_BASE;
    use hyperdrive::locks::mcs::McsLock;

    #[must_use]
    #[inline]
    pub fn check_kernel<S: MemSize>(page: Page<S>) -> bool {
        let start_vaddr = page.start_address();
        start_vaddr >= KERNEL_AS_BASE
    }

    #[must_use]
    #[inline]
    pub fn recursive_index() -> u16 {
        *KERNEL_PT_RECURSIVE_INDEX.get().unwrap()
    }

    #[inline]
    /// Get a reference to the currently active page table.
    ///
    /// # Safety
    ///
    /// The caller must only modify the kernel part of the page table.
    pub(in super::super) unsafe fn with_kernel_pt<R>(
        f: impl FnOnce(&mut PageTable<'static>) -> R,
    ) -> R {
        static KERNEL_PT_LOCK: McsLock<()> = McsLock::new(());

        let recursive_index = KERNEL_PT_RECURSIVE_INDEX.get().unwrap();
        let vaddr = VirtAddr::from_pt_indices(
            *recursive_index,
            *recursive_index,
            *recursive_index,
            *recursive_index,
            0,
        );
        let ptr = vaddr.as_mut_ptr::<Entries>();

        KERNEL_PT_LOCK.with_locked(|()| {
            // Safety: The page table given by the bootloader is valid
            let mut pt = PageTable::new(unsafe { &mut *ptr });
            f(&mut pt)
        })
    }

    #[must_use]
    #[inline]
    pub fn alloc_frame<S: MemSize>() -> Option<Frame<S>> {
        frame_alloc::with_frame_allocator(frame_alloc::FrameAllocator::alloc::<S>)
    }

    #[inline]
    pub fn free_frame<S: MemSize>(frame: Frame<S>) {
        frame_alloc::with_frame_allocator(|frame_allocator| frame_allocator.free(frame));
    }

    #[must_use]
    #[inline]
    pub fn reserve_pages<S: MemSize>(count: u64) -> Option<PageRangeInclusive<S>> {
        page_alloc::with_kernel_page_allocator(|palloc| palloc.allocate_pages(count))
    }

    #[inline]
    pub fn free_pages<S: MemSize>(pages: PageRangeInclusive<S>) {
        assert!(check_kernel(pages.start()));
        page_alloc::with_kernel_page_allocator(|palloc| palloc.free_pages(pages));
    }

    pub fn map_frame<S: MemSize>(
        page: Page<S>,
        frame: Frame<S>,
        flags: Flags,
    ) -> Result<(), MappingError<S>>
    where
        PageTable<'static>: Mapper<S, Flags>,
    {
        assert!(check_kernel(page));
        // Safety: We validated the page is in the kernel range.
        frame_alloc::with_frame_allocator(|fralloc| unsafe {
            with_kernel_pt(|pt| {
                pt.map(page, frame, flags | Flags::PRESENT, fralloc)
                    .map(|flush| flush.flush())
            })
        })
    }

    pub fn unmap_page<S: MemSize>(page: Page<S>) -> Result<Frame<S>, MappingError<S>>
    where
        PageTable<'static>: Mapper<S, Flags>,
    {
        assert!(check_kernel(page));
        // Safety: We validated the page is in the kernel range.
        unsafe {
            with_kernel_pt(|pt| {
                pt.unmap(page).map(|(frame, flush)| {
                    flush.flush();
                    frame
                })
            })
        }
    }

    #[must_use]
    pub fn alloc_map<S: MemSize>(size: usize, flags: Flags) -> Option<PageRangeInclusive<S>>
    where
        PageTable<'static>: Mapper<S, Flags>,
    {
        let pages = u64::try_from(size).unwrap().div_ceil(S::SIZE);
        let page_range = reserve_pages(pages)?;
        debug_assert!(check_kernel(page_range.start()));
        let start_page = page_range.start();
        let end_page = page_range.end();

        let mut mapped_end: Option<Page<S>> = None;

        // Safety: The page range is within the kernel range.
        let mapping_result = frame_alloc::with_frame_allocator(|frame_allocator| unsafe {
            with_kernel_pt(|page_table| {
                for page in Page::range_inclusive(start_page, end_page) {
                    let frame = frame_allocator
                        .alloc::<S>()
                        .ok_or(MappingError::FrameAllocationFailed)?;

                    let map_res =
                        page_table.map(page, frame, flags | Flags::PRESENT, frame_allocator);

                    match map_res {
                        Ok(flush) => {
                            flush.flush();
                            mapped_end = Some(page);
                        }
                        Err(err) => {
                            core::hint::cold_path();
                            frame_allocator.free(frame);
                            return Err(err);
                        }
                    }
                }
                Ok(())
            })
        });

        if mapping_result.is_err() {
            // Clean up
            if let Some(last_mapped_page) = mapped_end {
                for page in Page::range_inclusive(start_page, last_mapped_page) {
                    if let Ok(frame) = unmap_page(page) {
                        free_frame(frame);
                    }
                }
            }
            free_pages(page_range);
            return None;
        }

        Some(page_range)
    }

    #[must_use]
    pub fn alloc_map_zeroed<S: MemSize>(size: usize, flags: Flags) -> Option<PageRangeInclusive<S>>
    where
        PageTable<'static>: Mapper<S, Flags>,
    {
        let page_range = alloc_map::<S>(size, flags)?;

        unsafe {
            page_range
                .start()
                .start_address()
                .as_mut_ptr::<u8>()
                .write_bytes(0, size);
        }

        Some(page_range)
    }

    #[must_use]
    pub fn alloc_guarded_4k(count: u64, flags: Flags) -> Option<PageRangeInclusive<M4KiB>> {
        let (guard_start, page_range, guard_end) =
            page_alloc::with_kernel_page_allocator(|palloc| palloc.allocate_guarded(count))?;
        debug_assert!(check_kernel(page_range.start()));

        let start_page = page_range.start();
        let end_page = page_range.end();
        let mut mapped_end: Option<Page<M4KiB>> = None;

        // Safety: The page range is within the kernel range.
        let mapping_result = frame_alloc::with_frame_allocator(|frame_allocator| unsafe {
            with_kernel_pt(|page_table| {
                for page in Page::range_inclusive(start_page, end_page) {
                    let frame = frame_allocator
                        .alloc::<M4KiB>()
                        .ok_or(MappingError::FrameAllocationFailed)?;

                    let map_res =
                        page_table.map(page, frame, flags | Flags::PRESENT, frame_allocator);

                    match map_res {
                        Ok(flush) => {
                            flush.flush();
                            mapped_end = Some(page);
                        }
                        Err(err) => {
                            core::hint::cold_path();
                            frame_allocator.free(frame);
                            return Err(err);
                        }
                    }
                }
                Ok(())
            })
        });

        if mapping_result.is_err() {
            core::hint::cold_path();

            if let Some(last_mapped_page) = mapped_end {
                for page in Page::range_inclusive(start_page, last_mapped_page) {
                    if let Ok(frame) = unmap_page(page) {
                        free_frame(frame);
                    }
                }
            }
            free_pages(Page::range_inclusive(guard_start, guard_end));
            return None;
        }

        Some(page_range)
    }

    /// Unmap and free a kernel region.
    ///
    /// # Safety
    ///
    /// The caller must ensure the pages are no longer in use.
    pub unsafe fn unmap_free<S: MemSize>(page_range: PageRangeInclusive<S>)
    where
        PageTable<'static>: Mapper<S, Flags>,
    {
        assert!(check_kernel(page_range.start()));
        // Safety: The page range is within the kernel range.
        unsafe {
            with_kernel_pt(|page_table| {
                for page in page_range {
                    if let Ok((frame, flush)) = page_table.unmap(page) {
                        flush.flush();
                        free_frame(frame);
                    }
                }
            });
        }
        free_pages(page_range);
    }
}

pub mod process_local {
    use super::*;
    use beskar_core::arch::paging::Translator as _;
    use bootloader_api::KERNEL_AS_BASE;

    #[must_use]
    #[inline]
    fn check_user<S: MemSize>(page: Page<S>) -> bool {
        let vaddr = page.end_address();
        vaddr < KERNEL_AS_BASE
    }

    /// Shortcut for accessing the current process's page table.
    ///
    /// # Safety
    ///
    /// See `AddressSpace::with_page_table`.
    #[inline]
    unsafe fn with_current_pt<F: FnOnce(&mut PageTable<'static>) -> R, R>(f: F) -> R {
        unsafe { process::current().address_space().with_page_table(f) }
    }

    #[must_use]
    #[inline]
    pub fn reserve_pages<S: MemSize>(count: u64) -> Option<PageRangeInclusive<S>> {
        process::current()
            .address_space()
            .with_pgalloc(|palloc| palloc.allocate_pages(count))
    }

    pub fn map<S: MemSize>(
        page_range: PageRangeInclusive<S>,
        flags: Flags,
    ) -> Result<(), MappingError<S>>
    where
        PageTable<'static>: Mapper<S, Flags>,
    {
        assert!(check_user(page_range.end()));
        let mut mapped_end: Option<Page<S>> = None;

        // Safety: We validated the page range is in the userland range.
        let res = frame_alloc::with_frame_allocator(|frame_allocator| unsafe {
            with_current_pt(|pt| {
                for page in page_range {
                    let frame = frame_allocator
                        .alloc::<S>()
                        .ok_or(MappingError::FrameAllocationFailed)?;

                    let map_res = pt.map(page, frame, flags | Flags::PRESENT, frame_allocator);

                    match map_res {
                        Ok(flush) => {
                            flush.flush();
                            mapped_end = Some(page);
                        }
                        Err(err) => {
                            core::hint::cold_path();
                            frame_allocator.free(frame);
                            return Err(err);
                        }
                    }
                }

                Ok(())
            })
        });

        if res.is_err()
            && let Some(last_mapped_page) = mapped_end
        {
            frame_alloc::with_frame_allocator(|frame_allocator| {
                // Safety: We validated the page range is in the userland range.
                unsafe {
                    with_current_pt(|page_table| {
                        for page in Page::range_inclusive(page_range.start(), last_mapped_page) {
                            if let Ok((frame, flush)) = page_table.unmap(page) {
                                flush.flush();
                                frame_allocator.free(frame);
                            }
                        }
                    });
                }
            });
        }

        res
    }

    #[inline]
    fn free_pages<S: MemSize>(page_range: PageRangeInclusive<S>) {
        process::current()
            .address_space()
            .with_pgalloc(|palloc| palloc.free_pages(page_range));
    }

    #[must_use]
    pub fn alloc_guarded_4k(size: u64, flags: Flags) -> Option<PageRangeInclusive<M4KiB>> {
        let (guard_start, page_range, guard_end) = process::current()
            .address_space()
            .with_pgalloc(|palloc| palloc.allocate_guarded(size.div_ceil(M4KiB::SIZE)))?;

        if map(page_range, flags).is_err() {
            free_pages(Page::range_inclusive(guard_start, guard_end));
            return None;
        }

        Some(page_range)
    }

    #[must_use]
    #[inline]
    pub fn is_addr_owned(start: VirtAddr, end: VirtAddr) -> bool {
        process::current().address_space().is_addr_owned(start, end)
    }

    #[must_use]
    #[inline]
    pub fn alloc_map<S: MemSize>(size: usize, flags: Flags) -> Option<PageRangeInclusive<S>>
    where
        PageTable<'static>: Mapper<S, Flags>,
    {
        let page_count = u64::try_from(size).unwrap().div_ceil(S::SIZE);
        let page_range = reserve_pages::<S>(page_count)?;

        if map(page_range, flags).is_err() {
            free_pages(page_range);
            return None;
        }

        Some(page_range)
    }

    /// Unmap and free a process-local region.
    ///
    /// # Safety
    ///
    /// The caller must ensure the pages are no longer in use.
    pub unsafe fn unmap_free<S: MemSize>(page_range: PageRangeInclusive<S>)
    where
        PageTable<'static>: Mapper<S, Flags>,
    {
        assert!(check_user(page_range.end()));
        frame_alloc::with_frame_allocator(|frame_allocator| {
            // Safety: We validated the page range is in the userland range.
            unsafe {
                with_current_pt(|page_table| {
                    for page in page_range {
                        if let Ok((frame, flush)) = page_table.unmap(page) {
                            flush.flush();
                            frame_allocator.free(frame);
                        }
                    }
                });
            }
        });

        free_pages(page_range);
    }

    pub fn update_flags<S: MemSize>(
        page_range: PageRangeInclusive<S>,
        flags: Flags,
    ) -> Result<(), MappingError<S>>
    where
        PageTable<'static>: Mapper<S, Flags>,
    {
        assert!(check_user(page_range.end()));
        unsafe {
            with_current_pt(|pt| {
                for page in page_range {
                    let cache_flush = pt.update_flags(page, flags)?;
                    cache_flush.flush();
                }
                Ok(())
            })
        }
    }

    #[inline]
    pub fn map_frame<S: MemSize>(
        page: Page<S>,
        frame: Frame<S>,
        flags: Flags,
    ) -> Result<(), MappingError<S>>
    where
        PageTable<'static>: Mapper<S, Flags>,
    {
        assert!(check_user(page));
        // Safety: We validated the page is in the userland range.
        frame_alloc::with_frame_allocator(|frame_allocator| unsafe {
            with_current_pt(|pt| {
                pt.map(page, frame, flags | Flags::PRESENT, frame_allocator)
                    .map(|flush| flush.flush())
            })
        })
    }

    pub fn unmap_page<S: MemSize>(page: Page<S>) -> Result<Frame<S>, MappingError<S>>
    where
        PageTable<'static>: Mapper<S, Flags>,
    {
        assert!(check_user(page));
        // Safety: We validated the page range is in the userland range.
        unsafe {
            with_current_pt(|pt| {
                pt.unmap(page).map(|(frame, flush)| {
                    flush.flush();
                    frame
                })
            })
        }
    }

    #[must_use]
    #[inline]
    fn checked_user_range(start: VirtAddr, len: usize) -> bool {
        let len = u64::try_from(len).unwrap();
        let Some(end) = VirtAddr::try_new(start.as_u64().saturating_add(len - 1)) else {
            return false;
        };
        process::current().address_space().is_addr_owned(start, end)
    }

    fn probe_copy(
        user_addr: VirtAddr,
        len: usize,
        required_flags: Flags,
        mut copy_chunk: impl FnMut(VirtAddr, usize, usize),
    ) -> Result<(), MappingError<M4KiB>> {
        if !checked_user_range(user_addr, len) {
            return Err(MappingError::NotMapped);
        }

        let mut current = user_addr;
        unsafe {
            with_current_pt(|pt| {
                let mut checked = 0;

                while checked < len {
                    let offset = usize::from(current.page_offset());
                    let chunk_len =
                        (len - checked).min(usize::try_from(M4KiB::SIZE).unwrap() - offset);

                    let valid = pt
                        .translate_addr(current)
                        .is_some_and(|(_, flags)| flags.contains(required_flags));
                    if !valid {
                        return Err(MappingError::NotMapped);
                    }

                    checked += chunk_len;
                    current += u64::try_from(chunk_len).unwrap();
                }

                current = user_addr;
                let mut copied = 0;

                while copied < len {
                    let offset = usize::from(current.page_offset());
                    let chunk_len =
                        (len - copied).min(usize::try_from(M4KiB::SIZE).unwrap() - offset);

                    copy_chunk(current, copied, chunk_len);

                    copied += chunk_len;
                    current += u64::try_from(chunk_len).unwrap();
                }

                Ok(())
            })
        }
    }

    #[expect(dead_code, reason = "TODO")]
    /// Copy bytes from the current process' user address space into a kernel buffer.
    ///
    /// The whole source range must be mapped, present, and user-accessible. The
    /// current page table is held locked while the range is validated and copied.
    pub fn probe_read(dst: &mut [u8], src: VirtAddr) -> Result<(), MappingError<M4KiB>> {
        probe_copy(
            src,
            dst.len(),
            Flags::PRESENT | Flags::USER_ACCESSIBLE,
            |current, copied, chunk_len| {
                let src = current.as_ptr::<u8>();
                let dst = unsafe { dst.as_mut_ptr().add(copied) };
                let count = chunk_len;
                unsafe { core::ptr::copy_nonoverlapping(src, dst, count) };
            },
        )
    }

    #[expect(dead_code, reason = "TODO")]
    /// Copy bytes from a kernel buffer into the current process' user address space.
    ///
    /// The whole destination range must be mapped, present, user-accessible, and
    /// writable. The current page table is held locked while the range is
    /// validated and copied.
    pub fn probe_write(dst: VirtAddr, src: &[u8]) -> Result<(), MappingError<M4KiB>> {
        probe_copy(
            dst,
            src.len(),
            Flags::PRESENT | Flags::USER_ACCESSIBLE | Flags::WRITABLE,
            |current, copied, chunk_len| {
                let src = unsafe { src.as_ptr().add(copied) };
                let dst = current.as_mut_ptr::<u8>();
                let count = chunk_len;
                unsafe { core::ptr::copy_nonoverlapping(src, dst, count) };
            },
        )
    }
}
