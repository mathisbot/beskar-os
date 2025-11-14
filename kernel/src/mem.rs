use beskar_core::mem::ranges::MemoryRange;
use bootloader_api::KernelInfo;

pub mod address_space;
pub mod frame_alloc;
mod heap;
pub mod page_alloc;

pub fn init(recursive_index: u16, regions: &[MemoryRange], kernel_info: &KernelInfo) {
    frame_alloc::init(regions);
    address_space::init(recursive_index, kernel_info);
    heap::init();
}
