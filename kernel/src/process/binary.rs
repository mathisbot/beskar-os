mod elf;

use beskar_core::{arch::VirtAddr, process::binary::BinaryResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum BinaryType {
    Elf,
}

pub struct Binary<'a> {
    input: &'a [u8],
    kind: BinaryType,
}

impl<'a> Binary<'a> {
    #[must_use]
    #[inline]
    pub const fn new(input: &'a [u8], kind: BinaryType) -> Self {
        Self { input, kind }
    }

    /// Load the binary into memory.
    ///
    /// # Errors
    ///
    /// Returns an error if the binary is not valid.
    pub fn load(&self) -> BinaryResult<LoadedBinary> {
        match self.kind {
            BinaryType::Elf => elf::load(self.input),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct LoadedBinary {
    entry_point: extern "C" fn(),
    tls_template: Option<TlsTemplate>,
    // TODO: Add information about the binary, such as frames, to impl `Drop`
}

impl LoadedBinary {
    #[must_use]
    #[inline]
    pub const fn entry_point(&self) -> extern "C" fn() {
        self.entry_point
    }

    #[must_use]
    #[inline]
    pub const fn tls_template(&self) -> Option<TlsTemplate> {
        self.tls_template
    }
}

#[derive(Debug, Clone, Copy)]
/// TLS template for the binary.
pub struct TlsTemplate {
    start: VirtAddr,
    file_size: u64,
    mem_size: u64,
}

impl TlsTemplate {
    #[must_use]
    #[inline]
    pub const fn start(&self) -> VirtAddr {
        self.start
    }

    #[must_use]
    #[inline]
    pub const fn file_size(&self) -> u64 {
        self.file_size
    }

    #[must_use]
    #[inline]
    pub const fn mem_size(&self) -> u64 {
        self.mem_size
    }
}
