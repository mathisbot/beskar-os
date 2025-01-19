use core::{cell::UnsafeCell, mem::MaybeUninit};

use x86_64::{
    instructions::tables::load_tss,
    registers::segmentation::{CS, Segment},
    structures::{gdt::GlobalDescriptorTable, tss::TaskStateSegment},
};

use crate::{
    arch::commons::paging::{M4KiB, MemSize as _, Page},
    mem::{frame_alloc, page_alloc},
};

use super::paging::page_table::Flags;

pub const DOUBLE_FAULT_IST: u16 = 0;
pub const PAGE_FAULT_IST: u16 = 1;

#[derive(Debug)]
pub struct Gdt {
    // This field is needed to escalate from a borrow to a mutable borrow
    inner: UnsafeCell<InnerGdt>,
}

impl Gdt {
    #[must_use]
    pub const fn uninit() -> Self {
        Self {
            inner: UnsafeCell::new(InnerGdt {
                gdt: MaybeUninit::uninit(),
                tss: MaybeUninit::uninit(),
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

            let kernel_code_selector =
                gdt.append(x86_64::structures::gdt::Descriptor::kernel_code_segment());
            let _kernel_data_selector =
                gdt.append(x86_64::structures::gdt::Descriptor::kernel_data_segment());

            let _user_code_selector =
                gdt.append(x86_64::structures::gdt::Descriptor::user_code_segment());
            let _user_data_selector =
                gdt.append(x86_64::structures::gdt::Descriptor::user_data_segment());

            let tss_selector = gdt.append(x86_64::structures::gdt::Descriptor::tss_segment(tss));

            (gdt, (kernel_code_selector, tss_selector))
        };

        let gdt = inner.gdt.write(gdt);
        gdt.load();

        unsafe {
            CS::set_reg(segments.0);
            load_tss(segments.1);
        }
    }

    #[must_use]
    fn create_tss() -> TaskStateSegment {
        let mut tss = TaskStateSegment::new();
        tss.interrupt_stack_table[DOUBLE_FAULT_IST as usize] = {
            let (page_range, guard_page) = page_alloc::with_page_allocator(|page_allocator| {
                page_allocator.allocate_guarded::<M4KiB>(2).unwrap()
            });

            frame_alloc::with_frame_allocator(|frame_allocator| {
                frame_allocator.map_pages(
                    page_range,
                    Flags::WRITABLE | Flags::PRESENT | Flags::NO_EXECUTE,
                );
                frame_allocator.map_pages(
                    Page::range_inclusive(guard_page, guard_page),
                    Flags::PRESENT | Flags::NO_EXECUTE,
                );
            });

            x86_64::VirtAddr::new(
                (page_range.end.start_address().as_u64() + (M4KiB::SIZE - 1)) & !0xF,
            )
        };
        tss.interrupt_stack_table[PAGE_FAULT_IST as usize] = {
            let (page_range, guard_page) = page_alloc::with_page_allocator(|page_allocator| {
                page_allocator.allocate_guarded::<M4KiB>(2).unwrap()
            });

            frame_alloc::with_frame_allocator(|frame_allocator| {
                frame_allocator.map_pages(
                    page_range,
                    Flags::WRITABLE | Flags::PRESENT | Flags::NO_EXECUTE,
                );
                frame_allocator.map_pages(
                    Page::range_inclusive(guard_page, guard_page),
                    Flags::PRESENT | Flags::NO_EXECUTE,
                );
            });

            x86_64::VirtAddr::new(
                (page_range.end.start_address().as_u64() + (M4KiB::SIZE - 1)) & !0xF,
            )
        };
        tss
    }
}

#[derive(Debug)]
struct InnerGdt {
    gdt: MaybeUninit<GlobalDescriptorTable>,
    tss: MaybeUninit<TaskStateSegment>,
}
