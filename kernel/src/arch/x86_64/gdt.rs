use core::{cell::UnsafeCell, mem::MaybeUninit};

use x86_64::{
    registers::segmentation::{CS, Segment},
    structures::{
        gdt::{GlobalDescriptorTable, SegmentSelector},
        tss::TaskStateSegment,
    },
};

use crate::mem::{address_space, frame_alloc};
use beskar_core::arch::{
    commons::paging::{CacheFlush as _, M4KiB, Mapper as _, MemSize as _},
    x86_64::instructions::load_tss,
};

use beskar_core::arch::x86_64::paging::page_table::Flags;

pub const DOUBLE_FAULT_IST: u16 = 0;
pub const PAGE_FAULT_IST: u16 = 1;

const RSP0_STACK_PAGE_COUNT: u64 = 16; // 64 KiB

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
                kernel_code_selector: MaybeUninit::uninit(),
                kernel_data_selector: MaybeUninit::uninit(),
                user_code_selector: MaybeUninit::uninit(),
                user_data_selector: MaybeUninit::uninit(),
            }),
        }
    }

    pub fn init_load(&'static self) {
        // Safety:
        // Called only once per core on startup
        let inner = unsafe { &mut *self.inner.get() };

        let tss = inner.tss.write(Self::create_tss());

        let mut gdt = GlobalDescriptorTable::new();

        let kernel_code_selector =
            gdt.append(x86_64::structures::gdt::Descriptor::kernel_code_segment());
        let kernel_data_selector =
            gdt.append(x86_64::structures::gdt::Descriptor::kernel_data_segment());

        let user_data_selector =
            gdt.append(x86_64::structures::gdt::Descriptor::user_data_segment());
        let user_code_selector =
            gdt.append(x86_64::structures::gdt::Descriptor::user_code_segment());
        inner.kernel_code_selector.write(kernel_code_selector);
        inner.kernel_data_selector.write(kernel_data_selector);
        inner.user_data_selector.write(user_data_selector);
        inner.user_code_selector.write(user_code_selector);

        let tss_selector = gdt
            .append(x86_64::structures::gdt::Descriptor::tss_segment(tss))
            .0;

        let gdt = inner.gdt.write(gdt);
        gdt.load();

        unsafe {
            CS::set_reg(kernel_code_selector);
            load_tss(tss_selector);
        }
    }

    #[must_use]
    fn create_tss() -> TaskStateSegment {
        fn alloc_stack(count: u64) -> x86_64::VirtAddr {
            let (_guard_start, page_range, _guard_end) =
                address_space::with_kernel_pgalloc(|page_allocator| {
                    page_allocator.allocate_guarded(count).unwrap()
                });

            frame_alloc::with_frame_allocator(|frame_allocator| {
                crate::mem::address_space::with_kernel_pt(|page_table| {
                    for page in page_range {
                        let frame = frame_allocator.alloc::<M4KiB>().unwrap();
                        page_table
                            .map(
                                page,
                                frame,
                                Flags::PRESENT | Flags::WRITABLE | Flags::NO_EXECUTE,
                                frame_allocator,
                            )
                            .flush();
                    }
                });
            });

            x86_64::VirtAddr::new(page_range.end().start_address().as_u64() + (M4KiB::SIZE - 1))
                .align_down(16_u64)
        }

        let mut tss = TaskStateSegment::new();
        tss.interrupt_stack_table[DOUBLE_FAULT_IST as usize] = alloc_stack(2);
        tss.interrupt_stack_table[PAGE_FAULT_IST as usize] = alloc_stack(2);
        tss.privilege_stack_table[0] = alloc_stack(RSP0_STACK_PAGE_COUNT);
        tss
    }

    #[must_use]
    #[inline]
    pub fn kernel_code_selector(&self) -> SegmentSelector {
        let inner = unsafe { &*self.inner.get() };
        unsafe { inner.kernel_code_selector.assume_init() }
    }

    #[must_use]
    #[inline]
    pub fn kernel_data_selector(&self) -> SegmentSelector {
        let inner = unsafe { &*self.inner.get() };
        unsafe { inner.kernel_data_selector.assume_init() }
    }

    #[must_use]
    #[inline]
    pub fn user_code_selector(&self) -> SegmentSelector {
        let inner = unsafe { &*self.inner.get() };
        unsafe { inner.user_code_selector.assume_init() }
    }

    #[must_use]
    #[inline]
    pub fn user_data_selector(&self) -> SegmentSelector {
        let inner = unsafe { &*self.inner.get() };
        unsafe { inner.user_data_selector.assume_init() }
    }
}

#[derive(Debug)]
struct InnerGdt {
    gdt: MaybeUninit<GlobalDescriptorTable>,
    tss: MaybeUninit<TaskStateSegment>,
    kernel_code_selector: MaybeUninit<SegmentSelector>,
    kernel_data_selector: MaybeUninit<SegmentSelector>,
    user_code_selector: MaybeUninit<SegmentSelector>,
    user_data_selector: MaybeUninit<SegmentSelector>,
}
