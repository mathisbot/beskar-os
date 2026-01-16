use ::storage::{
    fs::{PathBuf, dev::DeviceFS},
    vfs::{Vfs, VfsHelper},
};
use alloc::boxed::Box;

struct VfsHelperStruct;

impl VfsHelper for VfsHelperStruct {
    #[inline]
    fn get_current_process_id() -> u64 {
        crate::process::current().pid().as_u64()
    }
}

static VFS: Vfs<VfsHelperStruct> = Vfs::new();

pub fn init() {
    let mut device_fs = DeviceFS::new();
    device_fs.add_device(
        PathBuf::new("/keyboard"),
        Box::new(crate::drivers::keyboard::KeyboardDevice),
    );
    device_fs.add_device(PathBuf::new("/stdout"), Box::new(crate::process::Stdout));
    device_fs.add_device(PathBuf::new("/rand"), Box::new(crate::process::RandFile));
    device_fs.add_device(
        PathBuf::new("/randseed"),
        Box::new(crate::process::SeedFile),
    );
    device_fs.add_device(PathBuf::new("/fb"), Box::new(video::screen::ScreenDevice));
    VFS.mount(PathBuf::new("/dev"), Box::new(device_fs));

    // TODO: Mount RAM disk (FAT32)
    // VFS.mount(PathBuf::new("/ramdisk"), todo!());
}

#[must_use]
#[inline]
/// Returns a reference to the global VFS instance.
pub fn vfs() -> &'static Vfs<impl VfsHelper> {
    &VFS
}
