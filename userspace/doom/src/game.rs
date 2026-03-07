use core::{
    ffi::{CStr, c_char, c_void},
    sync::atomic::{AtomicUsize, Ordering},
};

#[link(name = "puredoom", kind = "static")]
unsafe extern "C" {
    unsafe fn doom_set_print(f: extern "C" fn(*const c_char));
    unsafe fn doom_set_malloc(
        malloc: extern "C" fn(i32) -> *mut c_void,
        free: extern "C" fn(*mut c_void),
    );
    unsafe fn doom_set_file_io(
        open: extern "C" fn(*const c_char, *const c_char) -> *const c_void,
        close: extern "C" fn(*const c_void),
        read: extern "C" fn(*const c_void, *mut c_void, i32) -> i32,
        write: extern "C" fn(*const c_void, *const c_void, i32) -> i32,
        seek: extern "C" fn(*const c_void, i32, DoomSeekT) -> i32,
        tell: extern "C" fn(*const c_void) -> i32,
        eof: extern "C" fn(*const c_void) -> i32,
    );
    unsafe fn doom_set_gettime(f: extern "C" fn(*mut i32, *mut i32));
    unsafe fn doom_set_exit(f: extern "C" fn(i32));
    unsafe fn doom_set_getenv(f: extern "C" fn(*const c_char) -> *const c_char);
}

pub fn init() {
    unsafe { doom_set_print(print) };
    unsafe { doom_set_malloc(malloc, free) };
    unsafe { doom_set_file_io(open, close, read, write, seek, tell, eof) };
    unsafe { doom_set_gettime(gettime) };
    unsafe { doom_set_exit(exit) };
    unsafe { doom_set_getenv(getenv) };
}

extern "C" fn print(s: *const c_char) {
    if !cfg!(debug_assertions) {
        let s = unsafe { CStr::from_ptr(s) };
        if let Ok(s) = s.to_str() {
            beskar_lib::println!("DOOM: {}", s);
        }
    }
}

extern "C" fn malloc(size: i32) -> *mut c_void {
    // `alloc` states the layout size must be non-zero.
    if size == 0 {
        return core::ptr::dangling_mut();
    }

    let size = usize::try_from(size).unwrap();
    let layout =
        core::alloc::Layout::from_size_align(size, core::mem::align_of::<*const ()>()).unwrap();

    let ptr = unsafe { alloc::alloc::alloc(layout) };

    assert!(
        !ptr.is_null(),
        "malloc failed: out of memory (requested {size} bytes)",
    );

    ptr.cast()
}

const extern "C" fn free(_ptr: *mut c_void) {
    // TODO: keep track of allocations to get the layout!
}

extern "C" fn exit(code: i32) {
    let code = if code == 0 {
        beskar_lib::ExitCode::Success
    } else {
        beskar_lib::ExitCode::Failure
    };
    beskar_lib::exit(code);
}

extern "C" fn gettime(sec: *mut i32, usec: *mut i32) {
    let now = beskar_lib::time::now();
    unsafe {
        *sec = i32::try_from(now.secs()).unwrap();
        *usec = i32::try_from(now.micros()).unwrap();
    }
}

extern "C" fn getenv(name: *const c_char) -> *const c_char {
    let name = unsafe { core::ffi::CStr::from_ptr(name) };

    if let Ok(name) = name.to_str()
        && name == "HOME"
    {
        let dir = c"/";
        return dir.as_ptr();
    }

    core::ptr::null()
}

const WAD_HANDLE: *const c_void = 1 as _;

static WAD_POS: AtomicUsize = AtomicUsize::new(0);
static WAD: &[u8] = include_bytes!("../DOOM/doom1.wad");

extern "C" fn open(filename: *const c_char, _mode: *const c_char) -> *const c_void {
    let filename = unsafe { core::ffi::CStr::from_ptr(filename) };

    if let Ok(filename) = filename.to_str()
        && filename == "./doom1.wad"
    {
        return WAD_HANDLE;
    }

    core::ptr::null()
}

extern "C" fn close(handle: *const c_void) {
    assert_eq!(handle, WAD_HANDLE);
}

extern "C" fn read(handle: *const c_void, buf: *mut c_void, len: i32) -> i32 {
    if handle == WAD_HANDLE {
        let length = usize::try_from(len).unwrap();
        let start_offset = WAD_POS.fetch_add(length, Ordering::Relaxed);

        if start_offset + length > WAD.len() {
            return -1;
        }

        unsafe {
            core::ptr::copy_nonoverlapping(WAD.as_ptr().byte_add(start_offset), buf.cast(), length);
        }
        return len;
    }
    -1
}

const extern "C" fn write(_handle: *const c_void, _buf: *const c_void, _len: i32) -> i32 {
    -1
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
#[allow(dead_code)]
enum DoomSeekT {
    Set = 0,
    Cur = 1,
    End = 2,
}

extern "C" fn seek(handle: *const c_void, offset: i32, whence: DoomSeekT) -> i32 {
    if handle != WAD_HANDLE {
        return -1;
    }

    match whence {
        DoomSeekT::Set => {
            let Ok(offset) = usize::try_from(offset) else {
                return -1;
            };
            WAD_POS.store(offset, Ordering::Relaxed);
            0
        }
        DoomSeekT::Cur => {
            let current_pos = WAD_POS.load(Ordering::Relaxed) as isize;
            let new_pos = current_pos.saturating_add(offset as isize);
            if new_pos < 0 || new_pos as usize > WAD.len() {
                return -1;
            }
            WAD_POS.store(new_pos as usize, Ordering::Relaxed);
            0
        }
        DoomSeekT::End => {
            let end = WAD.len() as isize;
            let pos = (end + (offset as isize)).clamp(0, end);
            WAD_POS.store(pos as usize, Ordering::Relaxed);
            0
        }
    }
}

extern "C" fn tell(handle: *const c_void) -> i32 {
    if handle != WAD_HANDLE {
        return -1;
    }

    i32::try_from(WAD_POS.load(Ordering::Acquire)).unwrap()
}

extern "C" fn eof(handle: *const c_void) -> i32 {
    if handle != WAD_HANDLE {
        return -1;
    }

    let pos = WAD_POS.load(Ordering::Acquire);
    i32::from(pos >= WAD.len())
}
