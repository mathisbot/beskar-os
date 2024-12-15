use crate::{
    cpu, locals,
    mem::{
        frame_alloc, page_alloc, page_table,
        ranges::{MemoryRange, MemoryRangeRequest, MemoryRanges},
    },
};

use core::sync::atomic::{AtomicU64, Ordering};

use x86_64::{
    registers::control::{Cr0, Cr3, Cr4, Efer},
    structures::paging::{Mapper, Page, PageSize, PageTableFlags, PhysFrame, Size4KiB},
    PhysAddr, VirtAddr,
};

static AP_STACK_TOP_ADDR: AtomicU64 = AtomicU64::new(0);

/// Physical address of the AP trampoline code
///
/// Make sure that it is identity-mapped and at most 16-bit,
/// so that it can be read and accessed from APs (real-mode).
///
/// Here, `0x8000` is chosen as it is the first address that doesn't triggers a triple fault.
const AP_TRAMPOLINE_PADDR: u64 = 0x8000;

static BSP_CR0: AtomicU64 = AtomicU64::new(0);
static BSP_CR4: AtomicU64 = AtomicU64::new(0);
static BSP_EFER: AtomicU64 = AtomicU64::new(0);

/// AP trampoline code
///
/// Manually compiled with nasm from `ap_tramp.asm`.
/// Must be manually recompiled if the code changes.
const AP_TRAMPOLINE_CODE: &[u8] = include_bytes!("ap_tramp");

// TODO: If the main core panics, all APs should stop.
pub fn start_up_aps(core_count: u8) {
    // Store the current state of the BSP
    BSP_CR0.store(
        x86_64::registers::control::Cr0::read_raw(),
        Ordering::Release,
    );
    BSP_CR4.store(
        x86_64::registers::control::Cr4::read_raw(),
        Ordering::Release,
    );
    BSP_EFER.store(
        x86_64::registers::model_specific::Efer::read_raw(),
        Ordering::Release,
    );

    // Identity-map AP trampoline code, as paging isn't enabled on APs yet.
    // Everything should still be accessible with the same address when paging is enabled.

    let payload_paddr = PhysAddr::new(AP_TRAMPOLINE_PADDR);
    let frame = PhysFrame::<Size4KiB>::from_start_address(payload_paddr).unwrap();
    let payload_vaddr = VirtAddr::new(AP_TRAMPOLINE_PADDR);
    let page = Page::<Size4KiB>::from_start_address(payload_vaddr).unwrap();

    frame_alloc::with_frame_allocator(|frame_allocator| {
        page_table::with_page_table(|page_table| {
            unsafe {
                page_table.map_to(
                    page,
                    frame,
                    PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
                    &mut *frame_allocator,
                )
            }
            .unwrap()
            .flush();
        });
    });

    // Load code
    assert!(
        AP_TRAMPOLINE_CODE.len() <= usize::try_from(Size4KiB::SIZE).unwrap(),
        "AP trampoline code is too big"
    );
    unsafe {
        core::ptr::copy_nonoverlapping(
            AP_TRAMPOLINE_CODE.as_ptr(),
            payload_vaddr.as_mut_ptr(),
            AP_TRAMPOLINE_CODE.len(),
        );
    }

    // Update section .data of the AP trampoline code

    // Page table address
    let (frame, offset) = Cr3::read_raw();
    write_sipi(
        payload_vaddr,
        3,
        frame.start_address().as_u64() | u64::from(offset),
    );

    // Entry Point address
    write_sipi(payload_vaddr, 1, kap_entry as *const () as u64);

    // Base virtual address
    write_sipi(payload_vaddr, 0, payload_vaddr.as_u64());

    // Pointer to the address of the top of the stack
    // Note that using `as_ptr` in safe as the trampoline code uses atomic instructions
    write_sipi(payload_vaddr, 2, AP_STACK_TOP_ADDR.as_ptr() as u64);

    let sipi_payload = u8::try_from(payload_paddr.as_u64() >> 12).unwrap();

    // Wake up APs
    locals!().lapic().with_locked(|apic| {
        // FIXME: Decide if the following advised boot sequence is mandatory or if
        // this dumb code works just fine.
        // <https://wiki.osdev.org/Symmetric_Multiprocessing#Startup_Sequence>
        apic.send_sipi(None);
        // crate::time::tsc::wait_ms(10);
        apic.send_sipi(Some(sipi_payload));
        // crate::time::tsc::wait_ms(100);
        // apic.send_sipi(Some(sipi_payload));
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
        page_table::with_page_table(|page_table| {
            let (frame, tlb) = page_table.unmap(page).unwrap();
            tlb.flush();
            frame_allocator.free(frame);
        });
    });
    page_alloc::with_page_allocator(|page_allocator| {
        let page = Page::<Size4KiB>::from_start_address(payload_vaddr).unwrap();
        page_allocator.free_pages(Page::range_inclusive(page, page));
    });

    // Wait for APs to start
    while crate::locals::get_ready_core_count() != core_count {
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
        // The amount of pages should be kept in sync with the stack size allocated by the bootloader
        page_allocator.allocate_pages::<Size4KiB>(64).unwrap()
    });

    frame_alloc::with_frame_allocator(|frame_allocator| {
        frame_allocator.map_pages(
            stack_pages,
            PageTableFlags::WRITABLE | PageTableFlags::PRESENT | PageTableFlags::NO_EXECUTE,
        );
    });

    let stack_top = (stack_pages.end.start_address() + Size4KiB::SIZE - 1).align_down(16_u64);

    let previous_ap_stack = AP_STACK_TOP_ADDR.swap(stack_top.as_u64(), Ordering::SeqCst);
    assert_eq!(previous_ap_stack, 0, "AP stack allocated twice");
}

/// Reserve a frame for the AP trampoline code
///
/// It is easier to allocate the frame at the beginning of memory initialization,
/// because we are sure that the needed region is available.
pub fn reserve_tramp_frame(allocator: &mut frame_alloc::FrameAllocator) {
    let mut req_range = MemoryRanges::new();
    req_range.insert(MemoryRange::new(
        AP_TRAMPOLINE_PADDR,
        AP_TRAMPOLINE_PADDR + Size4KiB::SIZE,
    ));

    let _frame = allocator
        .alloc_request::<Size4KiB, 1>(&MemoryRangeRequest::MustBeWithin(&req_range))
        .expect("Failed to allocate AP frame");
}

/// Reserve a page for the AP trampoline code
///
/// It is easier to allocate the page at the beginning of memory initialization,
/// because we are sure that the needed region is available.
pub fn reserve_tramp_page(allocator: &mut page_alloc::PageAllocator) {
    let vaddr = VirtAddr::new(AP_TRAMPOLINE_PADDR);

    let page = Page::<Size4KiB>::from_start_address(vaddr).unwrap();

    assert!(
        allocator.allocate_specific_page(page).is_some(),
        "Failed to allocate AP page"
    );
}

/// Rust entry point for APs
///
/// This function is called by the AP trampoline code.
extern "C" fn kap_entry() -> ! {
    // Safety:
    // Values are coming from the BSP, so they are safe to use.
    unsafe {
        Cr0::write_raw(BSP_CR0.load(Ordering::Acquire));
        Cr4::write_raw(BSP_CR4.load(Ordering::Acquire));
        Efer::write_raw(BSP_EFER.load(Ordering::Acquire));
    }

    // Tell the BSP we are out of the trampoline spin lock,
    // allowing others to get their stack
    crate::locals::core_jumped();

    ap_init();

    log::debug!("Core {} ready!", locals!().core_id());

    crate::boot::enter_kmain()
}

fn ap_init() {
    cpu::init();

    locals::init();

    locals!().gdt().init_load();

    crate::cpu::interrupts::init();

    crate::cpu::apic::init_lapic();
}
