/// The ring of the CPU that the code is running in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ring {
    /// Ring 0 - Most privileged
    Kernel,
    /// Ring 3 - Least privileged
    User,
}
