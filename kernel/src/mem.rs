use beskar_core::{boot::KernelInfo, mem::MemoryRegion};

pub mod address_space;
pub mod frame_alloc;
mod heap;
pub mod page_alloc;

pub fn init(recursive_index: u16, regions: &[MemoryRegion], kernel_info: &KernelInfo) {
    address_space::init(recursive_index, kernel_info);

    frame_alloc::init(regions);

    page_alloc::init(recursive_index);

    heap::init();
}
