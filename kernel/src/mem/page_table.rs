use hyperdrive::locks::mcs::{MUMcsGuard, MUMcsLock, McsNode};
use x86_64::{
    registers::control::Cr3,
    structures::paging::{RecursivePageTable, Translate},
    VirtAddr,
};

static KERNEL_PAGE_TABLE: MUMcsLock<RecursivePageTable> = MUMcsLock::uninit();

pub fn init(recursive_index: u16) {
    let (level_4_page_table, _) = Cr3::read();

    let bootloader_pt_vaddr = {
        let recursive_index = u64::from(recursive_index);
        let vaddr = (recursive_index << 39)
            | (recursive_index << 30)
            | (recursive_index << 21)
            | (recursive_index << 12);
        VirtAddr::new(vaddr)
    };

    // Safety: The page table given by the bootloader is valid
    let bootloader_pt = unsafe { &mut *bootloader_pt_vaddr.as_mut_ptr() };

    let recursive_page_table = RecursivePageTable::new(bootloader_pt).unwrap();

    debug_assert_eq!(
        recursive_page_table
            .translate_addr(bootloader_pt_vaddr)
            .unwrap(),
        level_4_page_table.start_address()
    );

    KERNEL_PAGE_TABLE.init(recursive_page_table);
}

/// Explicitly returns a guard to the kernel page table
///
/// If you only plan on operating on the page table once, you can use `with_page_table`
///
/// ## Safety
///
/// The node must be valid for `McsLock::lock`
pub(super) fn get_kernel_page_table(
    node: &mut McsNode,
) -> MUMcsGuard<'_, '_, RecursivePageTable<'static>> {
    KERNEL_PAGE_TABLE.lock(node)
}

#[inline]
/// Perform a single operation on the kernel page table
pub fn with_page_table<F, R>(f: F) -> R
where
    F: FnOnce(&mut RecursivePageTable) -> R,
{
    KERNEL_PAGE_TABLE.with_locked(f)
}
