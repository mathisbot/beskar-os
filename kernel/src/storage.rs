use ::storage::vfs::{Vfs, VfsHelper};
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
    // TODO: Mount RAM disk (FAT32)
    // VFS.mount(PathBuf::new("/ramdisk"), todo!());
}
