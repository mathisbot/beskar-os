use crate::locals;

pub unsafe fn enter_usermode(entry: *const ()) {
    let usermode_code_selector = locals!().gdt().user_code_selector().0;
    let usermode_data_selector = locals!().gdt().user_data_selector().0;

    unsafe {
        core::arch::asm!(
            "mov rax, rsp",
            "push {0:x}",
            "push rax",
            "pushfq",
            "push {1:x}",
            "push {2:x}",
            "iretq",
            in(reg) usermode_data_selector,
            in(reg) usermode_code_selector,
            in(reg) entry,
        )
    }
}
