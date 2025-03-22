pub mod fs;
pub mod partition;
pub mod vfs;

pub fn init() {
    vfs::init();
}
