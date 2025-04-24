use core::mem::MaybeUninit;

use x86_64::structures::{
    gdt::{GlobalDescriptorTable, SegmentSelector},
    tss::TaskStateSegment,
};

use crate::mem::{address_space, frame_alloc};
use beskar_core::arch::{
    commons::paging::{CacheFlush as _, M4KiB, Mapper as _, MemSize as _},
    x86_64::{instructions::load_tss, registers::CS},
};

use beskar_core::arch::x86_64::paging::page_table::Flags;

pub const DOUBLE_FAULT_IST: u8 = 0;
pub const PAGE_FAULT_IST: u8 = 1;

const RSP0_STACK_PAGE_COUNT: u64 = 16; // 64 KiB

pub struct Gdt {
    loaded: bool,
    gdt: MaybeUninit<GlobalDescriptorTable>,
    tss: MaybeUninit<TaskStateSegment>,
    kernel_code_selector: MaybeUninit<SegmentSelector>,
    kernel_data_selector: MaybeUninit<SegmentSelector>,
    user_code_selector: MaybeUninit<SegmentSelector>,
    user_data_selector: MaybeUninit<SegmentSelector>,
}

impl Gdt {
    #[must_use]
    #[inline]
    pub const fn uninit() -> Self {
        Self {
            loaded: false,
            gdt: MaybeUninit::uninit(),
            tss: MaybeUninit::uninit(),
            kernel_code_selector: MaybeUninit::uninit(),
            kernel_data_selector: MaybeUninit::uninit(),
            user_code_selector: MaybeUninit::uninit(),
            user_data_selector: MaybeUninit::uninit(),
        }
    }

    /// Initializes the GDT and TSS and load them.
    ///
    /// # Safety
    ///
    /// `self` must be valid for `'static` and remain valid afterwards.
    pub unsafe fn init_load(&mut self) {
        let tss = Self::create_tss();

        let mut gdt = GlobalDescriptorTable::new();

        let kernel_code_selector =
            gdt.append(x86_64::structures::gdt::Descriptor::kernel_code_segment());
        let kernel_data_selector =
            gdt.append(x86_64::structures::gdt::Descriptor::kernel_data_segment());

        let user_data_selector =
            gdt.append(x86_64::structures::gdt::Descriptor::user_data_segment());
        let user_code_selector =
            gdt.append(x86_64::structures::gdt::Descriptor::user_code_segment());

        self.gdt.write(gdt);
        self.tss.write(tss);
        self.kernel_code_selector.write(kernel_code_selector);
        self.kernel_data_selector.write(kernel_data_selector);
        self.user_code_selector.write(user_code_selector);
        self.user_data_selector.write(user_data_selector);

        // Safety: We just initialized the GDT.
        // According to function's safety guards, `self` is valid for `'static`.
        let gdt = unsafe { &mut *core::ptr::from_mut(self.gdt.assume_init_mut()) };

        // Safety: We just initialized the TSS.
        // According to function's safety guards, `self` is valid for `'static`.
        let tss = unsafe { &*core::ptr::from_ref(self.tss.assume_init_ref()) };
        let tss_selector = gdt
            .append(x86_64::structures::gdt::Descriptor::tss_segment(tss))
            .0;

        gdt.load();

        unsafe {
            CS::set(kernel_code_selector.0);
            load_tss(tss_selector);
        }

        self.loaded = true;
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
    pub const fn kernel_code_selector(&self) -> Option<SegmentSelector> {
        if self.loaded {
            Some(unsafe { self.kernel_code_selector.assume_init() })
        } else {
            None
        }
    }

    #[must_use]
    #[inline]
    pub const fn kernel_data_selector(&self) -> Option<SegmentSelector> {
        if self.loaded {
            Some(unsafe { self.kernel_data_selector.assume_init() })
        } else {
            None
        }
    }

    #[must_use]
    #[inline]
    pub const fn user_code_selector(&self) -> Option<SegmentSelector> {
        if self.loaded {
            Some(unsafe { self.user_code_selector.assume_init() })
        } else {
            None
        }
    }

    #[must_use]
    #[inline]
    pub const fn user_data_selector(&self) -> Option<SegmentSelector> {
        if self.loaded {
            Some(unsafe { self.user_data_selector.assume_init() })
        } else {
            None
        }
    }

    #[must_use]
    #[inline]
    pub const fn tss(&self) -> Option<&TaskStateSegment> {
        if self.loaded {
            Some(unsafe { self.tss.assume_init_ref() })
        } else {
            None
        }
    }

    #[must_use]
    #[inline]
    pub const fn tss_mut(&mut self) -> Option<&mut TaskStateSegment> {
        if self.loaded {
            Some(unsafe { self.tss.assume_init_mut() })
        } else {
            None
        }
    }

    #[must_use]
    #[inline]
    pub const fn gdt(&self) -> Option<&GlobalDescriptorTable> {
        if self.loaded {
            Some(unsafe { self.gdt.assume_init_ref() })
        } else {
            None
        }
    }
}
