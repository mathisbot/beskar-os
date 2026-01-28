//! ELF segment structures and metadata.

use beskar_core::arch::VirtAddr;

/// Template for Thread-Local Storage initialization
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TlsTemplate {
    /// Virtual address of the TLS template
    pub start: VirtAddr,
    /// Size of initialized data in the template
    pub file_size: u64,
    /// Total size allocated for TLS
    pub mem_size: u64,
}

/// Information about a loaded ELF binary
#[derive(Debug, Clone)]
pub struct LoadedBinary {
    /// Entry point function pointer
    pub entry_point: extern "C" fn(),
    /// TLS template (if present)
    pub tls_template: Option<TlsTemplate>,
    /// Image size in memory
    pub image_size: u64,
}
