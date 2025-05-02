use beskar_core::process::ProcessId;
use storage::{
    fs::{FileError, FileResult, FileSystem, Path, PathBuf},
    vfs::{Vfs, VfsHelper},
};

static VFS: Vfs<MockVFSHelper> = Vfs::new();

struct MockFile {
    name: String,
    content: Vec<u8>,
}

struct MockFS {
    files: Vec<MockFile>,
}

impl FileSystem for MockFS {
    fn read(&mut self, path: Path, buffer: &mut [u8], offset: usize) -> FileResult<usize> {
        for file in &self.files {
            if file.name == path.as_str() {
                let bytes_to_read = core::cmp::min(buffer.len(), file.content.len() - offset);
                buffer[..bytes_to_read]
                    .copy_from_slice(&file.content[offset..offset + bytes_to_read]);
                return Ok(bytes_to_read);
            }
        }
        Err(FileError::NotFound)
    }

    fn write(&mut self, path: Path, buffer: &[u8], offset: usize) -> FileResult<usize> {
        for file in &mut self.files {
            if file.name == path.as_str() {
                // Make sure the file has enough space for the write.
                if file.content.len() < offset + buffer.len() {
                    file.content.resize(offset + buffer.len(), 0);
                }
                file.content[offset..offset + buffer.len()].copy_from_slice(buffer);
                return Ok(buffer.len());
            }
        }
        Err(FileError::NotFound)
    }

    fn close(&mut self, path: Path) -> FileResult<()> {
        for file in &self.files {
            if file.name == path.as_str() {
                // In a real filesystem, this would do something meaningful.
                return Ok(());
            }
        }
        Err(FileError::NotFound)
    }

    fn create(&mut self, path: Path) -> FileResult<()> {
        if self.files.iter().any(|file| file.name == path.as_str()) {
            return Err(FileError::AlreadyExists);
        }
        self.files.push(MockFile {
            name: String::from(path.as_str()),
            content: Vec::new(),
        });
        Ok(())
    }

    fn delete(&mut self, path: Path) -> FileResult<()> {
        if let Some(pos) = self
            .files
            .iter()
            .position(|file| file.name == path.as_str())
        {
            self.files.remove(pos);
            return Ok(());
        }
        Err(FileError::NotFound)
    }

    fn exists(&mut self, path: Path) -> FileResult<bool> {
        for file in &self.files {
            if file.name == path.as_str() {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn open(&mut self, path: Path) -> FileResult<()> {
        for file in &self.files {
            if file.name == path.as_str() {
                // In a real filesystem, this would do something meaningful.
                return Ok(());
            }
        }
        Err(FileError::NotFound)
    }
}

struct MockVFSHelper;

impl VfsHelper for MockVFSHelper {
    fn get_current_process_id() -> ProcessId {
        // Mock implementation, returning a dummy process ID.
        unsafe { ProcessId::from_raw(0) }
    }
}

#[test]
fn mock() {
    // Initialize the VFS with a mock filesystem.
    let fs = MockFS { files: Vec::new() };
    VFS.mount(PathBuf::new("/"), Box::new(fs));

    // Create a file.
    VFS.create(Path::from("/test.txt")).unwrap();

    // Check if the file exists.
    assert!(VFS.exists(Path::from("/test.txt")).unwrap());
    assert!(!VFS.exists(Path::from("/nonexistent.txt")).unwrap());

    // Open the file.
    let handle = VFS.open(Path::from("/test.txt")).unwrap();

    // Write to the file.
    let data = b"Hello, world!";
    VFS.write(handle, data, 0).unwrap();

    // Read from the file.
    let mut buffer = [0; 13];
    VFS.read(handle, &mut buffer, 0).unwrap();
    assert_eq!(&buffer, data);

    // Try to delete the file.
    assert!(VFS.delete(Path::from("/test.txt")).is_err());

    // Close the file.
    VFS.close(handle).unwrap();

    // Check if the file still exists.
    assert!(VFS.exists(Path::from("/test.txt")).unwrap());

    // Delete the file.
    VFS.delete(Path::from("/test.txt")).unwrap();
    assert!(!VFS.exists(Path::from("/test.txt")).unwrap());
}
