use core::{cell::UnsafeCell, mem::MaybeUninit};

use x86_64::{
    instructions::tables::load_tss,
    registers::segmentation::{Segment, CS},
    structures::{
        gdt::GlobalDescriptorTable,
        paging::{Page, PageSize, PageTableFlags, Size4KiB},
        tss::TaskStateSegment,
    },
};

use crate::mem::{frame_alloc, page_alloc};

pub const DOUBLE_FAULT_IST: u16 = 0;
pub const PAGE_FAULT_IST: u16 = 1;

#[derive(Debug)]
pub struct Gdt {
    // This field is needed to escalate from a borrow to a mutable borrow
    inner: UnsafeCell<InnerGdt>,
}

impl Gdt {
    pub const fn uninit() -> Self {
        Self {
            inner: UnsafeCell::new(InnerGdt {
                gdt: MaybeUninit::uninit(),
                tss: MaybeUninit::uninit(),
                // code_selector: MaybeUninit::uninit(),
                // tss_selector: MaybeUninit::uninit(),
            }),
        }
    }

    pub fn init_load(&'static self) {
        // Safety:
        // Called only once per core on startup
        let inner = unsafe { &mut *self.inner.get() };

        let tss = inner.tss.write(Self::create_tss());

        let (gdt, segments) = {
            let mut gdt = GlobalDescriptorTable::new();
            let code_selector =
                gdt.append(x86_64::structures::gdt::Descriptor::kernel_code_segment());
            let tss_selector = gdt.append(x86_64::structures::gdt::Descriptor::tss_segment(tss));
            (gdt, (code_selector, tss_selector))
        };

        let gdt = inner.gdt.write(gdt);
        // let code_selecor = inner.code_selector.write(segments.0);
        // let tss_selector = inner.tss_selector.write(segments.1);

        gdt.load();

        unsafe {
            CS::set_reg(segments.0);
            load_tss(segments.1);
        }
    }

    fn create_tss() -> TaskStateSegment {
        let mut tss = TaskStateSegment::new();
        tss.interrupt_stack_table[DOUBLE_FAULT_IST as usize] = {
            let (page_range, guard_page) = page_alloc::with_page_allocator(|page_allocator| {
                page_allocator.allocate_guarded::<Size4KiB>(2).unwrap()
            });

            frame_alloc::with_frame_allocator(|frame_allocator| {
                frame_allocator.map_pages(
                    page_range,
                    PageTableFlags::WRITABLE | PageTableFlags::PRESENT | PageTableFlags::NO_EXECUTE,
                );
                frame_allocator.map_pages(
                    Page::range_inclusive(guard_page, guard_page),
                    PageTableFlags::PRESENT | PageTableFlags::NO_EXECUTE,
                );
            });

            (page_range.end.start_address() + (Size4KiB::SIZE - 1)).align_down(16_u64)
        };
        tss.interrupt_stack_table[PAGE_FAULT_IST as usize] = {
            let (page_range, guard_page) = page_alloc::with_page_allocator(|page_allocator| {
                page_allocator.allocate_guarded::<Size4KiB>(2).unwrap()
            });

            frame_alloc::with_frame_allocator(|frame_allocator| {
                frame_allocator.map_pages(
                    page_range,
                    PageTableFlags::WRITABLE | PageTableFlags::PRESENT | PageTableFlags::NO_EXECUTE,
                );
                frame_allocator.map_pages(
                    Page::range_inclusive(guard_page, guard_page),
                    PageTableFlags::PRESENT | PageTableFlags::NO_EXECUTE,
                );
            });

            (page_range.end.start_address() + (Size4KiB::SIZE - 1)).align_down(16_u64)
        };
        tss
    }
}

#[derive(Debug)]
struct InnerGdt {
    gdt: MaybeUninit<GlobalDescriptorTable>,
    tss: MaybeUninit<TaskStateSegment>,
    // code_selector: MaybeUninit<SegmentSelector>,
    // tss_selector: MaybeUninit<SegmentSelector>,
}
