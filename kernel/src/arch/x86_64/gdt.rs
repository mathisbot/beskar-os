use crate::mem::vmm;
use beskar_core::arch::{
    Alignment, VirtAddr,
    paging::{M4KiB, MemSize as _},
};
use beskar_hal::{
    instructions::load_tss,
    paging::page_table::Flags,
    registers::CS,
    structures::{GdtDescriptor, GlobalDescriptorTable, TaskStateSegment},
};
use core::mem::MaybeUninit;

pub const DOUBLE_FAULT_IST: u8 = 0;
pub const PAGE_FAULT_IST: u8 = 1;

pub struct Gdt {
    loaded: bool,
    inner: MaybeUninit<GlobalDescriptorTable>,
    tss: MaybeUninit<TaskStateSegment>,
    kernel_code_selector: MaybeUninit<u16>,
    kernel_data_selector: MaybeUninit<u16>,
    user_code_selector: MaybeUninit<u16>,
    user_data_selector: MaybeUninit<u16>,
}

impl Gdt {
    #[must_use]
    #[inline]
    pub const fn uninit() -> Self {
        Self {
            loaded: false,
            inner: MaybeUninit::uninit(),
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

        let mut gdt = GlobalDescriptorTable::empty();

        let kernel_code_selector = gdt.append(GdtDescriptor::kernel_code_segment());
        let kernel_data_selector = gdt.append(GdtDescriptor::kernel_data_segment());

        let user_data_selector = gdt.append(GdtDescriptor::user_data_segment());
        let user_code_selector = gdt.append(GdtDescriptor::user_code_segment());

        self.inner.write(gdt);
        self.tss.write(tss);
        self.kernel_code_selector.write(kernel_code_selector);
        self.kernel_data_selector.write(kernel_data_selector);
        self.user_code_selector.write(user_code_selector);
        self.user_data_selector.write(user_data_selector);

        // Safety: We just initialized the GDT.
        // According to function's safety guards, `self` is valid for `'static`.
        let gdt = unsafe { &mut *core::ptr::from_mut(self.inner.assume_init_mut()) };

        // Safety: We just initialized the TSS.
        // According to function's safety guards, `self` is valid for `'static`.
        let tss = unsafe { &*core::ptr::from_ref(self.tss.assume_init_ref()) };
        let tss_selector = gdt.append(GdtDescriptor::tss_segment(tss));

        gdt.load();

        unsafe {
            CS::set(kernel_code_selector);
            load_tss(tss_selector);
        }

        self.loaded = true;
    }

    #[must_use]
    fn create_tss() -> TaskStateSegment {
        fn alloc_stack(count: u64) -> VirtAddr {
            let page_range = vmm::kernel::alloc_guarded_4k(
                count,
                Flags::PRESENT | Flags::WRITABLE | Flags::NO_EXECUTE,
            )
            .expect("Failed to allocate TSS stack");

            (page_range.end().start_address() + (M4KiB::SIZE - 1)).aligned_down(Alignment::Align16)
        }

        let mut tss = TaskStateSegment::new();
        tss.interrupt_stack_table[DOUBLE_FAULT_IST as usize] = alloc_stack(4);
        tss.interrupt_stack_table[PAGE_FAULT_IST as usize] = alloc_stack(4);

        tss
    }

    #[must_use]
    #[inline]
    pub const fn kernel_code_selector(&self) -> Option<u16> {
        if self.loaded {
            Some(unsafe { self.kernel_code_selector.assume_init() })
        } else {
            None
        }
    }

    #[must_use]
    #[inline]
    pub const fn kernel_data_selector(&self) -> Option<u16> {
        if self.loaded {
            Some(unsafe { self.kernel_data_selector.assume_init() })
        } else {
            None
        }
    }

    #[must_use]
    #[inline]
    pub const fn user_code_selector(&self) -> Option<u16> {
        if self.loaded {
            Some(unsafe { self.user_code_selector.assume_init() })
        } else {
            None
        }
    }

    #[must_use]
    #[inline]
    pub const fn user_data_selector(&self) -> Option<u16> {
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
            Some(unsafe { self.inner.assume_init_ref() })
        } else {
            None
        }
    }
}
