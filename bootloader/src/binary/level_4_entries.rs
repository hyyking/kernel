use core::convert::TryInto;

use libx64::{
    address::VirtualAddr,
    paging::{
        page::Page,
        table::{Level4, PageTableIndex},
        Page1Gb, Page4Kb,
    },
};

use xmas_elf::program::ProgramHeader;

/// Keeps track of used entries in a level 4 page table.
///
/// Useful for determining a free virtual memory block, e.g. for mapping additional data.
pub struct UsedLevel4Entries {
    entry_state: [bool; 512], // whether an entry is in use by the kernel
}

impl UsedLevel4Entries {
    /// Initializes a new instance from the given ELF program segments.
    ///
    /// Marks the virtual address range of all segments as used.
    pub fn new<'a>(
        segments: impl Iterator<Item = ProgramHeader<'a>>,
        virtual_address_offset: u64,
    ) -> Self {
        let mut used = UsedLevel4Entries {
            entry_state: [false; 512],
        };

        used.entry_state[0] = true; // TODO: Can we do this dynamically?

        for segment in segments {
            let start_page = Page::<Page4Kb>::containing(VirtualAddr::new(
                segment.virtual_addr() + virtual_address_offset,
            ));
            let end_page = Page::<Page4Kb>::containing(VirtualAddr::new(
                segment.virtual_addr() + virtual_address_offset + segment.mem_size(),
            ));

            let p4_start = start_page.ptr().page_table_index(Level4).value();
            let p4_end = end_page.ptr().page_table_index(Level4).value();

            for p4_index in p4_start..=p4_end {
                used.entry_state[p4_index as usize] = true;
            }
        }

        used
    }

    /// Returns a unused level 4 entry and marks it as used.
    ///
    /// Since this method marks each returned index as used, it can be used multiple times
    /// to determine multiple unused virtual memory regions.
    pub fn get_free_entry(&mut self) -> PageTableIndex<Level4> {
        let (idx, entry) = self
            .entry_state
            .iter_mut()
            .enumerate()
            .find(|(_, &mut entry)| entry == false)
            .expect("no usable level 4 entries found");

        *entry = true;
        PageTableIndex::new_truncate(idx.try_into().unwrap())
    }

    /// Returns the virtual start address of an unused level 4 entry and marks it as used.
    ///
    /// This is a convenience method around [`get_free_entry`], so all of its docs applies here
    /// too.
    pub fn get_free_address(&mut self) -> VirtualAddr {
        let idx = self.get_free_entry();
        Page::<Page1Gb>::containing(VirtualAddr::new((idx.value() as u64) << 39)).ptr()
        //       Page::from_page_table_indices_1gib(idx, PageTableIndex::new(0))
        //          .start_address()
    }
}
