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
