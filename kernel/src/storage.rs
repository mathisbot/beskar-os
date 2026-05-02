use ::storage::{
    fs::{PathBuf, dev::DeviceFS},
    vfs::Vfs,
};
use alloc::boxed::Box;
use hyperdrive::once::Once;

static VFS: Once<Vfs> = Once::uninit();

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
    let dev_mount_res = vfs.mount(PathBuf::new("/dev"), Box::new(device_fs));
    if dev_mount_res.is_err() {
        crate::warn!("Failed to mount /dev filesystem");
    }

    VFS.call_once(|| vfs);
}

#[must_use]
#[inline]
/// Returns a reference to the global VFS instance.
pub fn vfs() -> &'static Vfs {
    VFS.get().expect("VFS not initialized")
}
