#![no_main]
#![no_std]
#![warn(clippy::pedantic, clippy::nursery)]

#[cfg(not(target_arch = "x86_64"))]
compile_error!("BeskarOS bootloader only supports x86_64 architecture");

use boot::MemoryType;
use bootloader::mem::EarlyFrameAllocator;
use log::{debug, error, info, trace, warn};
use uefi::{
    mem::memory_map::{MemoryMap, MemoryMapMut},
    prelude::*,
    proto::{
        console::gop::{GraphicsOutput, PixelFormat},
        pi::mp::MpServices,
    },
};
use x86_64::{
    PhysAddr, VirtAddr,
    structures::paging::{FrameAllocator, OffsetPageTable, PageSize, PageTable, Size4KiB},
};

#[panic_handler]
/// Handles panics in the bootloader by logging an error message and attempting
/// to either gracefully reset the system or hang if necessary.
fn panic(panic_info: &core::panic::PanicInfo) -> ! {
    error!("[PANIC]: {}", panic_info.message());

    // If in debug mode, delay to allow the user to read the message
    #[cfg(debug_assertions)]
    {
        if let Some(location) = panic_info.location() {
            error!(
                "Panic occured in file '{}' at line {}",
                location.file(),
                location.line()
            );
        }

        // Check if boot services are still active
        let boot_service_active = uefi::table::system_table_raw()
            .is_some_and(|system_table| !unsafe { system_table.as_ref() }.boot_services.is_null());

        // Stall for a significant amount of time to allow the user to read the message
        if boot_service_active {
            boot::stall(5_000_000);
        } else {
            let mut x = 0_u64;
            for i in 0..100_000_000 {
                unsafe {
                    core::ptr::write_volatile(&mut x, i);
                }
            }
        }
    }

    // Check if runtime services are available (they sould be)
    let runtime_services_available = uefi::table::system_table_raw()
        .is_some_and(|system_table| !unsafe { system_table.as_ref() }.runtime_services.is_null());

    // If possible, gracefully shutdown.
    // Otherwise, hang the system.
    if runtime_services_available {
        uefi::runtime::reset(uefi::runtime::ResetType::COLD, uefi::Status::ABORTED, None);
    } else {
        loop {
            x86_64::instructions::hlt();
        }
    }
}

#[entry]
fn efi_entry() -> Status {
    uefi::helpers::init().unwrap();

    // In debug mode, disable the watchdog timer
    #[cfg(debug_assertions)]
    let _ = boot::set_watchdog_timer(0, 0, None);

    system::with_stdout(|stdout| {
        let _ = stdout.output_string(cstr16!("BeskarOS bootloader is starting\n"));
    });

    // Initialize the framebuffer and the logger using GOP (almost infaillible)
    // Framebuffer shouldn't be used (it is already used by the logger)
    let framebuffer = create_framebuffer_logger();

    // Print basic firmware information and check for compatibility
    check_system();

    // Handle multiple CPUs
    let cpu_count = enable_and_count_cores();

    #[cfg(debug_assertions)]
    debug!("Bootloader running in debug mode");

    // Load Kernel file in RAM
    //
    // Kernel is expected to be the only file named `kernelx64.elf` in the `efi` directory
    let kernel = {
        let file_content =
            fs::load_file_from_efi_dir(cstr16!("kernelx64.elf")).expect("Failed to load kernel");

        xmas_elf::ElfFile::new(file_content).expect("Failed to parse kernel")
    };
    info!("Kernel file loaded");

    // Exit boot services and get memory map
    let mut memory_map = unsafe { boot::exit_boot_services(boot::MemoryType::LOADER_DATA) };
    debug!("Boot services exited");
    memory_map.sort();

    if log::log_enabled!(log::Level::Debug) {
        let total_mem_size = memory_map
            .entries()
            .filter_map(|entry| match entry.ty {
                MemoryType::CONVENTIONAL
                | MemoryType::LOADER_CODE
                | MemoryType::LOADER_DATA
                | MemoryType::BOOT_SERVICES_CODE
                | MemoryType::BOOT_SERVICES_DATA
                | MemoryType::RUNTIME_SERVICES_CODE
                | MemoryType::RUNTIME_SERVICES_DATA => Some(entry.page_count),
                _ => None,
            })
            .sum::<u64>()
            * (Size4KiB::SIZE / 1024);
        debug!("Dected memory size: {} KiB", total_mem_size);
    }

    // Use that memory map to create a basic frame allocator
    let mut frame_allocator = bootloader::mem::EarlyFrameAllocator::new(memory_map);

    // Create page tables for bootloader, kernel and level 4 frame
    let page_tables = create_page_tables(&mut frame_allocator, &framebuffer);
    info!("Page tables created");

    // Find the hopefully available XSDP/RSDP
    let rsdp_paddr = uefi::system::with_config_table(|config_entries| {
        // Look for ACPI 2 XSDP first
        let acpi2_xsdp = config_entries
            .iter()
            .find(|config_entry| config_entry.guid == uefi::table::cfg::ACPI2_GUID);
        if acpi2_xsdp.is_some() {
            info!("ACPI 2.0 XSDP found");
        } else {
            debug!("ACPI 2.0 XSDP not found");
        }

        // If XSDP is not found, fallback to ACPI 1 RSDP
        let rsdp = acpi2_xsdp.or_else(|| {
            config_entries
                .iter()
                .find(|config_entry| config_entry.guid == uefi::table::cfg::ACPI_GUID)
        });
        if acpi2_xsdp.is_none() && rsdp.is_some() {
            info!("ACPI 1.0 RSDP found");
        } else if rsdp.is_none() {
            debug!("ACPI 1.0 RSDP not found neither");
        }

        rsdp.map(|entry| PhysAddr::new(entry.address as u64))
    });

    let system_info = bootloader::EarlySystemInfo {
        framebuffer,
        rsdp_paddr,
        cpu_count,
    };

    bootloader::map_memory_and_jump(&kernel, frame_allocator, page_tables, system_info)
}

/// Print firmware information and check for compatibility.
fn check_system() {
    info!("Firmware Vendor: {}", system::firmware_vendor());
    info!("Firmware Revision: {}", system::firmware_revision());

    let rev = system::uefi_revision();

    info!("UEFI specification: v{}.{}", rev.major(), rev.minor() / 10);

    assert_eq!(rev.major(), 2, "Running on an unsupported version of UEFI");
    if rev.minor() < 30 {
        warn!("Old version of UEFI 2, some features might not be available.");
    }
}

fn enable_and_count_cores() -> u8 {
    let mps = {
        let mp_handle = uefi::boot::get_handle_for_protocol::<MpServices>().unwrap();
        boot::open_protocol_exclusive::<MpServices>(mp_handle).unwrap()
    };

    debug!("Making sure all processors are enabled...");
    for i in 0..mps.get_number_of_processors().unwrap().total {
        if i != mps.who_am_i().unwrap() {
            let info = mps.get_processor_info(i).unwrap();
            if info.is_healthy() {
                mps.enable_disable_ap(i, true, Some(true)).unwrap();
            } else {
                warn!("Processor {} is not healthy, skipping it.", i);
                // Make sure it is disabled
                mps.enable_disable_ap(i, false, Some(false)).unwrap();
            }
        }
    }

    let proc_count = mps.get_number_of_processors().unwrap();
    if proc_count.enabled != proc_count.total {
        warn!(
            "Only {} out of {} processors could be enabled",
            proc_count.enabled, proc_count.total
        );
    }
    u8::try_from(proc_count.enabled).unwrap()
}

#[must_use]
/// Initializes the framebuffer logger for use by the bootloader.
///
/// The framebuffer is used to display log messages after boot services are exited.
/// This framebuffer will also be passed to the kernel for further use.
///
/// ## Note
///
/// The bootloader currently freezes the screen here if the screen only support `BltOnly` pixel format.
fn create_framebuffer_logger() -> bootloader::PhysicalFrameBuffer {
    let mut gop = {
        // Starting from UEFI 2.0, loacting GOP cannot fail.
        let gop_handle = uefi::boot::get_handle_for_protocol::<GraphicsOutput>().unwrap();
        // The handle is a valid `GraphicsOutput` protocol handle and
        // it is not opened anywhere else.
        boot::open_protocol_exclusive::<GraphicsOutput>(gop_handle).unwrap()
    };

    // Panics:
    // There is at least one mode (the current one) available.
    let best_mode = gop
        .modes()
        .max_by(|a, b| {
            // BltOnly pixel format is not supported
            if a.info().pixel_format() == PixelFormat::BltOnly {
                return core::cmp::Ordering::Less;
            }

            let res_a = a.info().resolution();
            let res_b = b.info().resolution();

            match res_a.0.cmp(&res_b.0) {
                core::cmp::Ordering::Equal => res_a.1.cmp(&res_b.1),
                other => other,
            }
        })
        .unwrap();

    let mode_info = best_mode.info();

    let pixel_format = match mode_info.pixel_format() {
        PixelFormat::Rgb => bootloader::PixelFormat::Rgb,
        PixelFormat::Bgr => bootloader::PixelFormat::Bgr,
        PixelFormat::Bitmask => {
            bootloader::PixelFormat::Bitmask(mode_info.pixel_bitmask().unwrap())
        }
        PixelFormat::BltOnly => {
            panic!("BltOnly pixel format is not supported");
        }
    };

    // Panics:
    // The mode is supported by the GOP as it is provided by `modes()`.
    gop.set_mode(&best_mode).unwrap();

    let mut framebuffer = gop.frame_buffer();

    // Safety:
    // The framebuffer address and buffer length are valid because they are derived
    // from the GOP-provided framebuffer, which guarantees their correctness.
    let framebuffer_slice =
        unsafe { core::slice::from_raw_parts_mut(framebuffer.as_mut_ptr(), framebuffer.size()) };

    let framebuffer_info = bootloader::FrameBufferInfo {
        size: framebuffer.size(),
        width: mode_info.resolution().0,
        height: mode_info.resolution().1,
        pixel_format,
        bytes_per_pixel: 4, // In every pixel format supported, each pixel is 4 bytes
        stride: mode_info.stride(),
    };

    let logger = bootloader::logging::init(framebuffer_slice, framebuffer_info);
    log::set_logger(logger).expect("Failed to set logger");
    log::set_max_level(if cfg!(debug_assertions) {
        log::LevelFilter::Trace
    } else {
        log::LevelFilter::Info
    });

    trace!(
        "Framebuffer initialized at {}x{}",
        framebuffer_info.width, framebuffer_info.height
    );

    // Safety:
    // The framebuffer address and information are valid because they are derived
    // from the GOP-provided framebuffer, which guarantees their correctness.
    unsafe {
        bootloader::PhysicalFrameBuffer::new(
            PhysAddr::new(framebuffer.as_mut_ptr() as u64),
            framebuffer_info,
        )
    }
}

mod fs {
    use boot::MemoryType;
    use uefi::{
        CStr16,
        data_types::Align,
        prelude::*,
        proto::media::file::{Directory, File, FileAttribute, FileHandle, FileInfo, FileMode},
    };
    use x86_64::structures::paging::{PageSize, Size4KiB};

    #[must_use]
    /// Loads a file from the filesystem.
    ///
    /// This function performs a depth-first search to find and load
    /// the first file that matches the given `filename`.
    /// Returns a mutable reference to the loaded file's contents or `None` if the file was not found.
    pub fn load_file_from_efi_dir(filename: &CStr16) -> Option<&'static mut [u8]> {
        let mut current_fs = boot::get_image_file_system(boot::image_handle()).unwrap();
        let mut root = current_fs.open_volume().unwrap();

        // Search for efi_dir
        let mut efi_dir: Directory = {
            let mut buffer = [0_u8; 512];
            let fi_buffer = FileInfo::align_buf(&mut buffer)?;

            let mut efi_dir: Option<Directory> = None;

            while let Ok(Some(file_info)) = root.read_entry(fi_buffer) {
                if file_info.is_directory() {
                    let name = file_info.file_name();
                    if name == cstr16!("efi") {
                        efi_dir = Some(
                            root.open(name, FileMode::Read, FileAttribute::default())
                                .ok()?
                                .into_directory()?,
                        );
                    }
                }
            }

            efi_dir?
        };

        let mut file_handle = {
            // Using the stack-allocated buffer as a parameter instead of allocating a new buffer
            // at reach recursive call of `find_file_in_dir` to avoid stack overflow.
            let mut buffer = [0_u8; 512];
            let fi_buffer = FileInfo::align_buf(&mut buffer)?;

            find_file_in_dir(&mut efi_dir, filename, fi_buffer)?.into_regular_file()?
        };

        let mut buffer = [0_u8; 512];
        let fi_buffer = FileInfo::align_buf(&mut buffer)?;

        let file_size = usize::try_from(
            file_handle
                .get_info::<FileInfo>(fi_buffer)
                .ok()?
                .file_size(),
        )
        .unwrap();

        let ptr = boot::allocate_pages(
            boot::AllocateType::AnyPages,
            MemoryType::LOADER_DATA,
            file_size.div_ceil(usize::try_from(Size4KiB::SIZE).unwrap()),
        )
        .ok()?;

        // Safety:
        // `ptr` is a valid pointer (`NonNull`) to an array of `u8` with length at least `file_size`
        let file_slice = unsafe { core::slice::from_raw_parts_mut(ptr.as_ptr(), file_size) };

        file_handle.read(file_slice).ok()?;

        Some(file_slice)
    }

    #[must_use]
    /// Finds the first file matching the requested filename in the directory
    /// and its subdirectories, according to a depth-first search algorithm.
    ///
    /// Returns `None` if the file is not found.
    fn find_file_in_dir(
        dir: &mut Directory,
        filename: &CStr16,
        fi_buffer: &mut [u8],
    ) -> Option<FileHandle> {
        while let Ok(Some(file_info)) = dir.read_entry(fi_buffer) {
            if file_info.is_directory() {
                let name = file_info.file_name();
                if name != cstr16!(".") && name != cstr16!("..") {
                    let mut subdir = dir
                        .open(name, FileMode::Read, FileAttribute::default())
                        .ok()?
                        .into_directory()?;
                    if let Some(file_handle) = find_file_in_dir(&mut subdir, filename, fi_buffer) {
                        return Some(file_handle);
                    }
                }
            } else if file_info.file_name() == filename {
                return dir
                    .open(filename, FileMode::Read, FileAttribute::default())
                    .ok();
            }
        }

        None
    }
}

fn create_page_tables(
    frame_allocator: &mut EarlyFrameAllocator,
    framebuffer: &bootloader::PhysicalFrameBuffer,
) -> bootloader::mem::PageTables {
    // All memory is identity mapped by UEFI
    let physical_offset = x86_64::VirtAddr::new(0);

    // TODO: Don't
    let bootloader_page_table = {
        let old_table = {
            let (old_frame, _) = x86_64::registers::control::Cr3::read();
            let ptr: *const PageTable =
                (physical_offset + old_frame.start_address().as_u64()).as_ptr();

            // ## Safety
            // We are reading a page table from a valid physical address mapped
            // in the virtual address space.
            unsafe { &*ptr }
        };

        let frame = frame_allocator
            .allocate_frame()
            .expect("Failed to allocate a frame");

        let table = {
            let ptr: *mut PageTable =
                (physical_offset + frame.start_address().as_u64()).as_mut_ptr();

            // ## Safety
            // We are writing a page table to a valid physical address
            // mapped in the virtual address space.
            unsafe {
                ptr.write(PageTable::new());
                &mut *ptr
            }
        };

        // Copy indexes for identity mapped memory
        let end_vaddr = VirtAddr::new(frame_allocator.max_physical_address().as_u64() - 1);
        for p4_index in 0..=usize::from(end_vaddr.p4_index()) {
            table[p4_index] = old_table[p4_index].clone();
        }

        // Copy indexes for framebuffer (which is not necessarily identity mapped)
        let start_vaddr = VirtAddr::new(framebuffer.buffer_start().as_u64());
        let end_vaddr = start_vaddr + u64::try_from(framebuffer.info().size).unwrap();
        for p4_index in usize::from(start_vaddr.p4_index())..=usize::from(end_vaddr.p4_index()) {
            table[p4_index] = old_table[p4_index].clone();
        }

        info!("Switching to a new level 4 page table");

        unsafe {
            x86_64::registers::control::Cr3::write(
                frame,
                x86_64::registers::control::Cr3Flags::empty(),
            );
            OffsetPageTable::new(&mut *table, physical_offset)
        }
    };

    // Create a new page table hierarchy for the kernel
    let (kernel_page_table, kernel_level_4_frame) = {
        let frame = frame_allocator
            .allocate_frame()
            .expect("Failed to allocate a frame");

        debug!(
            "Kernel level 4 page table is at {:#x}",
            frame.start_address().as_u64()
        );

        let ptr: *mut PageTable = (physical_offset + frame.start_address().as_u64()).as_mut_ptr();

        // Safety:
        // We are writing a page table to a valid physical address
        // mapped in the virtual address space.
        let table = unsafe {
            ptr.write(PageTable::new());
            &mut *ptr
        };

        (
            unsafe { OffsetPageTable::new(table, physical_offset) },
            frame,
        )
    };

    bootloader::mem::PageTables {
        bootloader: bootloader_page_table,
        kernel: kernel_page_table,
        kernel_level_4_frame,
    }
}
