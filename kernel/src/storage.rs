use ::storage::{
    fs::{PathBuf, dev::DeviceFS},
    vfs::{Vfs, VfsHelper},
};
use alloc::boxed::Box;
use beskar_core::process::ProcessId;

struct VfsHelperStruct;

impl VfsHelper for VfsHelperStruct {
    #[inline]
    fn get_current_process_id() -> ProcessId {
        crate::process::current().pid()
    }
}

static VFS: Vfs<VfsHelperStruct> = Vfs::new();

pub fn init() {
    let mut device_fs = DeviceFS::new();
    device_fs.add_device(
        PathBuf::new("/keyboard"),
        crate::drivers::keyboard::KeyboardDevice,
    );
    VFS.mount(PathBuf::new("/dev"), Box::new(device_fs));

    // TODO: Mount RAM disk (FAT32)
    // VFS.mount(PathBuf::new("/ramdisk"), todo!());
}

pub fn vfs() -> &'static Vfs<impl VfsHelper> {
    &VFS
}
