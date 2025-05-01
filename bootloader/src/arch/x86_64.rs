use beskar_core::arch::commons::{VirtAddr, paging::Frame};

pub mod acpi;

pub fn init() {
    // Find the hopefully available XSDP/RSDP
    acpi::init();
}

/// Change context and jump to the kernel entry point.
///
/// ## Safety
///
/// The caller must ensure that the four adresses are valid.
pub unsafe fn chg_ctx(
    level4_frame: Frame,
    stack_top: VirtAddr,
    entry_point_addr: VirtAddr,
    boot_info_addr: VirtAddr,
) -> ! {
    unsafe {
        core::arch::asm!(
            r#"
            xor rbp, rbp
            mov cr3, {}
            mov rsp, {}
            jmp {}
            "#,
            in(reg) level4_frame.start_address().as_u64(),
            in(reg) stack_top.as_u64(),
            in(reg) entry_point_addr.as_u64(),
            in("rdi") boot_info_addr.as_u64(),
            options(noreturn)
        )
    }
}
