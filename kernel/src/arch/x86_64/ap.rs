use super::apic::ipi::{self, Ipi};
use crate::{
    locals,
    mem::{address_space, frame_alloc, page_alloc},
};
use beskar_core::arch::{
    commons::{
        PhysAddr, VirtAddr,
        paging::{CacheFlush as _, Frame, M4KiB, Mapper as _, MemSize as _, Page},
    },
    x86_64::{
        paging::page_table::Flags,
        registers::{Cr0, Cr3, Cr4, Efer},
    },
};

use core::sync::atomic::{AtomicU64, Ordering};

// The amount of pages should be kept in sync with the bootloader
const KERNEL_STACK_NB_PAGES: u64 = 64; // 256 KiB

static AP_STACK_TOP_ADDR: AtomicU64 = AtomicU64::new(0);

/// Physical address of the AP trampoline code
///
/// Make sure that it is identity-mapped and at most 16-bit,
/// so that it can be read and accessed from APs (real-mode).
///
/// Here, `0x8000` is chosen as it is the first address that doesn't triggers a triple fault.
pub const AP_TRAMPOLINE_PADDR: u64 = 0x8000;

static BSP_CR0: AtomicU64 = AtomicU64::new(0);
static BSP_CR4: AtomicU64 = AtomicU64::new(0);
static BSP_EFER: AtomicU64 = AtomicU64::new(0);

/// AP trampoline code
///
/// Manually compiled with nasm from `ap_tramp.asm`.
/// Must be manually recompiled if the code changes.
const AP_TRAMPOLINE_CODE: &[u8] = include_bytes!("ap/ap_tramp");
beskar_core::static_assert!(
    AP_TRAMPOLINE_CODE.len() <= 4096,
    "AP trampoline code is too big"
);

pub fn start_up_aps(core_count: usize) {
    if core_count <= 1 {
        return;
    }

    // Store the current state of the BSP
    store_ap_regs();

    // Identity-map AP trampoline code, as paging isn't enabled on APs yet.
    // Everything should still be accessible with the same address when paging is enabled.

    let payload_paddr = PhysAddr::new(AP_TRAMPOLINE_PADDR);
    let frame = Frame::<M4KiB>::from_start_address(payload_paddr).unwrap();
    let payload_vaddr = VirtAddr::new(AP_TRAMPOLINE_PADDR);
    let page = Page::<M4KiB>::from_start_address(payload_vaddr).unwrap();

    frame_alloc::with_frame_allocator(|frame_allocator| {
        address_space::with_kernel_pt(|page_table| {
            page_table
                .map(
                    page,
                    frame,
                    Flags::PRESENT | Flags::WRITABLE,
                    &mut *frame_allocator,
                )
                .flush();
        });
    });

    // Load code
    unsafe {
        core::ptr::copy_nonoverlapping(
            AP_TRAMPOLINE_CODE.as_ptr(),
            payload_vaddr.as_mut_ptr(),
            AP_TRAMPOLINE_CODE.len(),
        );
    }

    // Update section .data of the AP trampoline code

    // Page table address
    let cr3_raw = Cr3::read_raw();
    write_sipi(payload_vaddr, 3, cr3_raw);

    // Entry Point address
    write_sipi(payload_vaddr, 1, crate::boot::kap_entry as u64);

    // Base virtual address
    write_sipi(payload_vaddr, 0, payload_vaddr.as_u64());

    // Pointer to the address of the top of the stack
    // Note that using `as_ptr` is safe as the trampoline code uses atomic instructions
    write_sipi(payload_vaddr, 2, AP_STACK_TOP_ADDR.as_ptr() as u64);

    let sipi_payload = u8::try_from(payload_paddr.as_u64() >> 12).unwrap();

    // Wake up APs
    locals!().lapic().with_locked(|apic| {
        apic.send_ipi(&Ipi::new(
            ipi::DeliveryMode::Init,
            ipi::Destination::AllExcludingSelf,
        ));
        // FIXME: Is it useful to wait a bit here?
        // crate::time::tsc::wait_ms(10);
        apic.send_ipi(&Ipi::new(
            ipi::DeliveryMode::Sipi(sipi_payload),
            ipi::Destination::AllExcludingSelf,
        ));
    });

    // Now, each AP will be waiting for a stack,
    // so we should give them one!
    for i in 1..core_count {
        allocate_stack();
        while locals::get_jumped_core_count() == i {
            // Wait for APs to jump (release stack spinlock)
            core::hint::spin_loop();
        }
    }

    // Free trampoline code
    frame_alloc::with_frame_allocator(|frame_allocator| {
        address_space::with_kernel_pt(|page_table| {
            let (frame, tlb) = page_table.unmap(page).unwrap();
            tlb.flush();
            frame_allocator.free(frame);
        });
    });
    page_alloc::with_page_allocator(|page_allocator| {
        let page = Page::<M4KiB>::from_start_address(payload_vaddr).unwrap();
        page_allocator.free_pages(Page::range_inclusive(page, page));
    });

    // Wait for APs to start
    while locals::get_ready_core_count() != core_count {
        core::hint::spin_loop();
    }
}

fn write_sipi(payload_vaddr: VirtAddr, offset_count: u64, value: u64) {
    let payload_end =
        payload_vaddr + AP_TRAMPOLINE_CODE.len() as u64 - u64::try_from(size_of::<u64>()).unwrap();
    let target = payload_end - offset_count * u64::try_from(size_of::<u64>()).unwrap();
    unsafe {
        target.as_mut_ptr::<u64>().write(value);
    }
}

fn allocate_stack() {
    let stack_pages = page_alloc::with_page_allocator(|page_allocator| {
        page_allocator
            .allocate_pages::<M4KiB>(KERNEL_STACK_NB_PAGES)
            .unwrap()
    });

    frame_alloc::with_frame_allocator(|frame_allocator| {
        crate::mem::address_space::with_kernel_pt(|page_table| {
            for page in stack_pages {
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

    let stack_top = (stack_pages.end.start_address() + M4KiB::SIZE - 1).align_down(16_u64);

    let previous_ap_stack = AP_STACK_TOP_ADDR.swap(stack_top.as_u64(), Ordering::SeqCst);
    assert_eq!(previous_ap_stack, 0, "AP stack allocated twice");
}

pub unsafe fn load_ap_regs() {
    unsafe {
        Cr0::write(BSP_CR0.load(Ordering::Relaxed));
        Cr4::write(BSP_CR4.load(Ordering::Relaxed));
        Efer::write(BSP_EFER.load(Ordering::Relaxed));
    }
}

pub fn store_ap_regs() {
    BSP_CR0.store(Cr0::read(), Ordering::Relaxed);
    BSP_CR4.store(Cr4::read(), Ordering::Relaxed);
    BSP_EFER.store(Efer::read(), Ordering::Relaxed);
}
