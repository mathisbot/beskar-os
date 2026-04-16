use crate::{core::TestResult, ensure, ensure_eq};
use beskar_lib::error::MemoryErrorKind;
use beskar_lib::mem::MemoryProtection;
use core::num::NonZeroU64;

pub fn test_memory_api() -> TestResult {
    mmap_invalid_alignment()?;
    mmap_write()?;
    mmap_read_mprotect_write()?;
    munmap()?;
    heap()?;

    Ok(())
}

fn mmap_invalid_alignment() -> TestResult {
    let bad_alignment = NonZeroU64::new(3).unwrap();
    match beskar_lib::mem::mmap(4096, Some(bad_alignment), MemoryProtection::ReadWrite) {
        Err(err) if matches!(err.kind(), MemoryErrorKind::InvalidAlignment) => Ok(()),
        _ => Err("mmap accepted invalid alignment"),
    }
}

fn mmap_write() -> TestResult {
    let mapping = beskar_lib::mem::mmap(4096, None, MemoryProtection::ReadWrite)
        .map_err(|_| "mmap failed")?;
    unsafe { mapping.write_volatile(0) };
    Ok(())
}

fn mmap_read_mprotect_write() -> TestResult {
    let mapping =
        beskar_lib::mem::mmap(4096, None, MemoryProtection::ReadOnly).map_err(|_| "mmap failed")?;
    ensure!(
        beskar_lib::mem::mprotect(mapping.as_ptr(), 4096, MemoryProtection::ReadWrite),
        "mprotect(read-only) failed",
    );
    unsafe { mapping.write_volatile(0) };
    Ok(())
}

fn munmap() -> TestResult {
    let mapping =
        beskar_lib::mem::mmap(4096, None, MemoryProtection::ReadOnly).map_err(|_| "mmap failed")?;
    ensure!(
        unsafe { beskar_lib::mem::munmap(mapping.as_ptr(), 4096) },
        "munmap failed",
    );
    Ok(())
}

fn heap() -> TestResult {
    let mut boxed = alloc::boxed::Box::new(42);
    ensure_eq!(*boxed, 42, "heap allocation did not return expected value");
    *boxed = 100;
    ensure_eq!(*boxed, 100, "heap allocation did not allow mutation");
    Ok(())
}
