use super::fs::{FileSystem, PathBuf};
use alloc::{boxed::Box, collections::BTreeMap};
use hyperdrive::locks::rw::RwLock;

static VFS: Vfs = Vfs::new();

pub fn init() {
    // TODO: Mount RAM disk (FAT32)
    // VFS.mount(PathBuf::new("/ramdisk"), todo!());
}

#[derive(Default)]
pub struct Vfs {
    mounts: RwLock<BTreeMap<PathBuf, RwLock<Box<dyn FileSystem>>>>,
}

impl Vfs {
    #[must_use]
    #[inline]
    pub const fn new() -> Self {
        Self {
            mounts: RwLock::new(BTreeMap::new()),
        }
    }

    pub fn mount(&self, path: PathBuf, fs: Box<dyn FileSystem>) {
        self.mounts.write().insert(path, RwLock::new(fs));
    }
}
