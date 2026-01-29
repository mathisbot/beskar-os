use ::storage::{
    fs::{PathBuf, dev::DeviceFS},
    vfs::{Vfs, VfsHelper},
};
use alloc::boxed::Box;
use hyperdrive::once::Once;

struct VfsHelperStruct;

impl VfsHelper for VfsHelperStruct {
    #[inline]
    fn get_current_process_id() -> u64 {
        crate::process::current().pid().as_u64()
    }
}

static VFS: Once<Vfs<VfsHelperStruct>> = Once::uninit();

pub fn init() {
    let vfs = Vfs::new();
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
    vfs.mount(PathBuf::new("/dev"), Box::new(device_fs));

    VFS.call_once(|| vfs);
}

#[must_use]
#[inline]
/// Returns a reference to the global VFS instance.
pub fn vfs() -> &'static Vfs<impl VfsHelper> {
    VFS.get().expect("VFS not initialized")
}
