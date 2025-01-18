#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entry(u64);

pub struct PageTable {
    entries: [Entry; 512],
    recursive_index: u16,
}
