#[repr(C)]
pub struct GlobalDescriptorTable {
    entries: [u8; 255],
    at: usize,
}

impl GlobalDescriptorTable {
    /// Get a reference to the global descriptor table's entries.
    pub fn entries(&self) -> &[u8] {
        &self.entries[..self.at]
    }
}
