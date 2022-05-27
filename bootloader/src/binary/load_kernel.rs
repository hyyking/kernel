use core::mem::align_of;

use crate::{binary::bootloader::Kernel, boot_info::TlsTemplate};

use libx64::{
    address::VirtualAddr,
    paging::{
        entry::Flags,
        frame::{FrameAllocator, FrameError, FrameRange, PhysicalFrame},
        page::{
            Page, PageMapper, PageRange, PageRangeInclusive, PageTranslator, TlbFlush, TlbMethod,
        },
        table::Translation,
        Page4Kb,
    },
};

use xmas_elf::{
    dynamic,
    program::{self, ProgramHeader, SegmentData, Type},
    sections::Rela,
    ElfFile,
};

/// Used by [`Inner::make_mut`] and [`Inner::clean_copied_flag`].
const COPIED: Flags = Flags::AVL1;

pub struct Loader<'a, F, M> {
    inner: Inner<'a, F, M>,
}

struct Inner<'a, F, M> {
    kernel: &'a Kernel,

    page_table: &'a mut M,
    frame_allocator: &'a mut F,
}

impl<'a, F, M> Loader<'a, F, M>
where
    F: FrameAllocator<Page4Kb>,
    M: PageMapper<Page4Kb> + PageTranslator,
{
    pub fn new(kernel: &'a Kernel, page_table: &'a mut M, frame_allocator: &'a mut F) -> Self {
        Loader {
            inner: Inner {
                kernel,

                page_table,
                frame_allocator,
            },
        }
    }

    pub fn load_segments(&mut self) -> Result<Option<TlsTemplate>, &'static str> {
        let elf_file = self.inner.kernel.elf_file();

        for program_header in elf_file.program_iter() {
            program::sanity_check(program_header, &elf_file)?;
        }

        // Load the segments into virtual memory.
        let mut tls_template = None;
        for program_header in elf_file.program_iter() {
            match program_header.get_type()? {
                Type::Load => self
                    .inner
                    .handle_load_segment(program_header)
                    .map_err(|_| "load failed")?,
                Type::Tls => {
                    if tls_template.is_none() {
                        tls_template = Some(self.inner.handle_tls_segment(program_header)?);
                    } else {
                        return Err("multiple TLS segments not supported");
                    }
                }
                a @ (Type::Null
                | Type::Dynamic
                | Type::Interp
                | Type::Note
                | Type::ShLib
                | Type::Phdr
                | Type::GnuRelro
                | Type::OsSpecific(_)
                | Type::ProcessorSpecific(_)) => {
                    warn!("Unsuported program header {:?}", a);
                }
            }
        }

        info!("Applying rellocations");

        // Apply relocations in virtual memory.
        for program_header in elf_file.program_iter() {
            if let Type::Dynamic = program_header.get_type()? {
                self.inner
                    .handle_dynamic_segment(program_header, &elf_file)?
            }
        }

        self.inner.remove_copied_flags(&elf_file).unwrap();

        Ok(tls_template)
    }
}

impl<'a, F, M> Inner<'a, F, M>
where
    F: FrameAllocator<Page4Kb>,
    M: PageMapper<Page4Kb> + PageTranslator,
{
    fn handle_load_segment(&mut self, segment: ProgramHeader) -> Result<(), FrameError> {
        info!(
            "Segment({:?}): {} ({}K)",
            segment.get_type().unwrap_or(xmas_elf::program::Type::Null),
            segment.flags(),
            segment.mem_size() as usize / Page4Kb
        );

        let phys_start_addr = self.kernel.start + segment.offset();
        let start_frame = PhysicalFrame::<Page4Kb>::containing(phys_start_addr);
        let end_frame = PhysicalFrame::<Page4Kb>::containing(
            (phys_start_addr + segment.file_size()).align_up(Page4Kb as u64),
        );

        let start_page = Page::<Page4Kb>::containing(self.kernel.offset() + segment.virtual_addr());
        let end_page = Page::<Page4Kb>::containing(
            (self.kernel.offset() + segment.virtual_addr() + segment.file_size())
                .align_up(Page4Kb as u64),
        );

        let mut segment_flags = Flags::PRESENT;

        /* NOTE: My cpu doesn't support EFER.NX see CPUID feature section 4.1.4 Intel manual
        if !segment.flags().is_execute() {
            segment_flags |= Flags::NX;
        }
        */

        if segment.flags().is_write() {
            segment_flags |= Flags::RW;
        }

        self.page_table.map_range(
            PageRange::new(start_page, end_page),
            FrameRange::new(start_frame, end_frame),
            segment_flags,
            self.frame_allocator,
            TlbMethod::Ignore,
        )?;
        /*
                // map all frames of the segment at the desired virtual address
                for frame in FrameRange::new(start_frame, end_frame) {
                    let offset = frame.ptr().as_u64() - start_frame.ptr().as_u64();
                    let page = Page::containing(VirtualAddr::new(start_page.ptr().as_u64() + offset));
                    self.page_table
                        .map(page, frame, segment_flags, self.frame_allocator)
                        .map(TlbFlush::ignore)?
                }
        */
        // Handle .bss section (mem_size > file_size)
        if segment.mem_size() > segment.file_size() {
            // .bss section (or similar), which needs to be mapped and zeroed
            self.handle_bss_section(&segment, segment_flags)?;
        }

        Ok(())
    }

    fn handle_bss_section(
        &mut self,
        segment: &ProgramHeader,
        segment_flags: Flags,
    ) -> Result<(), FrameError> {
        info!("Mapping bss section");

        let virt_start_addr = self.kernel.offset() + segment.virtual_addr();
        let mem_size = segment.mem_size();
        let file_size = segment.file_size();

        // calculate virual memory region that must be zeroed
        let zero_start = virt_start_addr + file_size;
        let zero_end = virt_start_addr + mem_size;

        // a type alias that helps in efficiently clearing a page
        type PageArray = [u8; Page4Kb / 8];
        const ZERO_ARRAY: PageArray = [0; Page4Kb / 8];

        // In some cases, `zero_start` might not be page-aligned. This requires some
        // special treatment because we can't safely zero a frame of the original file.
        let data_bytes_before_zero = zero_start.as_usize() & 0xfff;
        if data_bytes_before_zero != 0 {
            // The last non-bss frame of the segment consists partly of data and partly of bss
            // memory, which must be zeroed. Unfortunately, the file representation might have
            // reused the part of the frame that should be zeroed to store the next segment. This
            // means that we can't simply overwrite that part with zeroes, as we might overwrite
            // other data this way.
            //
            // Example:
            //
            //   XXXXXXXXXXXXXXX000000YYYYYYY000ZZZZZZZZZZZ     virtual memory (XYZ are data)
            //   |·············|     /·····/   /·········/
            //   |·············| ___/·····/   /·········/
            //   |·············|/·····/‾‾‾   /·········/
            //   |·············||·····|/·̅·̅·̅·̅·̅·····/‾‾‾‾
            //   XXXXXXXXXXXXXXXYYYYYYYZZZZZZZZZZZ              file memory (zeros are not saved)
            //   '       '       '       '        '
            //   The areas filled with dots (`·`) indicate a mapping between virtual and file
            //   memory. We see that the data regions `X`, `Y`, `Z` have a valid mapping, while
            //   the regions that are initialized with 0 have not.
            //
            //   The ticks (`'`) below the file memory line indicate the start of a new frame. We
            //   see that the last frames of the `X` and `Y` regions in the file are followed
            //   by the bytes of the next region. So we can't zero these parts of the frame
            //   because they are needed by other memory regions.
            //
            // To solve this problem, we need to allocate a new frame for the last segment page
            // and copy all data content of the original frame over. Afterwards, we can zero
            // the remaining part of the frame since the frame is no longer shared with other
            // segments now.

            let last_page = Page::<Page4Kb>::containing(virt_start_addr + file_size - 1u64);
            let new_frame = unsafe { self.make_mut(last_page)? };
            let new_bytes_ptr = new_frame.ptr().ptr::<u8>().unwrap().as_ptr();
            unsafe {
                core::ptr::write_bytes(
                    new_bytes_ptr.add(data_bytes_before_zero),
                    0,
                    Page4Kb - data_bytes_before_zero,
                );
            }
        }

        // map additional frames for `.bss` memory that is not present in source file
        let start_page = Page::<Page4Kb>::containing(
            VirtualAddr::new(zero_start.as_u64()).align_up(Page4Kb as u64),
        );
        let end_page = Page::<Page4Kb>::containing(zero_end);
        for page in PageRangeInclusive::new(start_page, end_page) {
            // allocate a new unused frame
            let frame = self.frame_allocator.alloc().unwrap();

            // zero frame, utilizing identity-mapping
            let frame_ptr = frame.ptr().as_u64() as *mut PageArray;
            unsafe { frame_ptr.write(ZERO_ARRAY) };

            // map frame
            self.page_table
                .map(page, frame, segment_flags, self.frame_allocator)
                .map(TlbFlush::ignore)?;
        }

        Ok(())
    }

    /// This method is intended for making the memory loaded by a Load segment mutable.
    ///
    /// All memory from a Load segment starts out by mapped to the same frames that
    /// contain the elf file. Thus writing to memory in that state will cause aliasing issues.
    /// To avoid that, we allocate a new frame, copy all bytes from the old frame to the new frame,
    /// and remap the page to the new frame. At this point the page no longer aliases the elf file
    /// and we can write to it.
    ///
    /// When we map the new frame we also set [`COPIED`] flag in the page table flags, so that
    /// we can detect if the frame has already been copied when we try to modify the page again.
    ///
    /// ## Safety
    /// - `page` should be a page mapped by a Load segment.
    ///  
    /// ## Panics
    /// Panics if the page is not mapped in `self.page_table`.
    unsafe fn make_mut(
        &mut self,
        page: Page<Page4Kb>,
    ) -> Result<PhysicalFrame<Page4Kb>, FrameError> {
        let (frame, flags) = match self.page_table.try_translate(page.ptr()) {
            Ok(Translation {
                addr,
                offset: _,
                flags,
            }) => (PhysicalFrame::<Page4Kb>::containing(addr), flags),
            Err(e) => panic!("{:?}", e),
        };

        if flags.contains(COPIED) {
            // The frame was already copied, we are free to modify it.
            return Ok(frame);
        }

        // Allocate a new frame and copy the memory, utilizing that both frames are identity mapped.
        let new_frame = self.frame_allocator.alloc().unwrap();
        let frame_ptr = frame.ptr().as_u64() as *const u8;
        let new_frame_ptr = new_frame.ptr().as_u64() as *mut u8;
        unsafe {
            core::ptr::copy_nonoverlapping(frame_ptr, new_frame_ptr, Page4Kb as usize);
        }

        // Replace the underlying frame and update the flags.
        self.page_table.unmap(page).map(TlbFlush::ignore)?;

        let new_flags = flags | COPIED;
        self.page_table
            .map(page, new_frame, new_flags, self.frame_allocator)
            .map(TlbFlush::ignore)?;

        Ok(new_frame)
    }

    /// Cleans up the custom flags set by [`Inner::make_mut`].
    fn remove_copied_flags(&mut self, elf_file: &ElfFile) -> Result<(), FrameError> {
        for program_header in elf_file
            .program_iter()
            .filter(|ph| ph.get_type().unwrap() == Type::Load)
        {
            let start = self.kernel.offset() + program_header.virtual_addr();
            let end = start + program_header.mem_size();
            let start_page = Page::<Page4Kb>::containing(start);
            let end_page = Page::<Page4Kb>::containing(end);

            for page in PageRange::new(start_page, end_page) {
                // Translate the page and get the flags.
                let Translation { flags, .. } = self.page_table.try_translate(page.ptr())?;

                if flags.contains(COPIED) {
                    // Remove the flag.
                    self.page_table
                        .update_flags(page, flags & !COPIED)
                        .map(TlbFlush::ignore)?;
                }
            }
        }
        Ok(())
    }

    fn handle_tls_segment(&mut self, segment: ProgramHeader) -> Result<TlsTemplate, &'static str> {
        Ok(TlsTemplate {
            start_addr: self.kernel.offset().as_u64() + segment.virtual_addr(),
            mem_size: segment.mem_size(),
            file_size: segment.file_size(),
        })
    }

    fn handle_dynamic_segment(
        &mut self,
        segment: ProgramHeader,
        elf_file: &ElfFile,
    ) -> Result<(), &'static str> {
        let data = segment.get_data(elf_file)?;
        let data = if let SegmentData::Dynamic64(data) = data {
            data
        } else {
            panic!("expected Dynamic64 segment")
        };

        // Find the `Rela`, `RelaSize` and `RelaEnt` entries.
        let mut rela = None;
        let mut rela_size = None;
        let mut rela_ent = None;
        for rel in data {
            let tag = rel.get_tag()?;
            match tag {
                dynamic::Tag::Rela => {
                    let ptr = rel.get_ptr()?;
                    let prev = rela.replace(ptr);
                    if prev.is_some() {
                        return Err("Dynamic section contains more than one Rela entry");
                    }
                }
                dynamic::Tag::RelaSize => {
                    let val = rel.get_val()?;
                    let prev = rela_size.replace(val);
                    if prev.is_some() {
                        return Err("Dynamic section contains more than one RelaSize entry");
                    }
                }
                dynamic::Tag::RelaEnt => {
                    let val = rel.get_val()?;
                    let prev = rela_ent.replace(val);
                    if prev.is_some() {
                        return Err("Dynamic section contains more than one RelaEnt entry");
                    }
                }
                _ => {}
            }
        }
        let offset = if let Some(rela) = rela {
            rela
        } else {
            // The section doesn't contain any relocations.

            if rela_size.is_some() || rela_ent.is_some() {
                return Err("Rela entry is missing but RelaSize or RelaEnt have been provided");
            }

            return Ok(());
        };
        let total_size = rela_size.ok_or("RelaSize entry is missing")?;
        let entry_size = rela_ent.ok_or("RelaEnt entry is missing")?;

        // Apply the mappings.
        let entries = (total_size / entry_size) as usize;
        let rela_start = elf_file
            .input
            .as_ptr()
            .wrapping_add(offset as usize)
            .cast::<Rela<u64>>();

        // Make sure the relocations are inside the elf file.
        let rela_end = rela_start.wrapping_add(entries);
        assert!(rela_start <= rela_end);
        let file_ptr_range = elf_file.input.as_ptr_range();
        assert!(
            file_ptr_range.start <= rela_start.cast(),
            "the relocation table must start in the elf file"
        );
        assert!(
            rela_end.cast() <= file_ptr_range.end,
            "the relocation table must end in the elf file"
        );

        let relas = unsafe { core::slice::from_raw_parts(rela_start, entries) };
        for rela in relas {
            let idx = rela.get_symbol_table_index();
            assert_eq!(
                idx, 0,
                "relocations using the symbol table are not supported"
            );

            match rela.get_type() {
                // R_AMD64_RELATIVE
                8 => {
                    check_is_in_load(elf_file, rela.get_offset())?;
                    let addr = self.kernel.offset() + rela.get_offset();
                    let value = self.kernel.offset() + rela.get_addend();

                    if addr.as_usize() % align_of::<u64>() != 0 {
                        return Err("destination of relocation is not aligned");
                    }

                    let page = Page::<Page4Kb>::containing(addr);
                    let offset_in_page = addr.as_u64() - page.ptr().as_u64();

                    let new_frame = unsafe { self.make_mut(page).map_err(|_| "frame error")? };
                    let phys_addr = new_frame.ptr() + offset_in_page;
                    let addr = phys_addr.as_u64() as *mut u64;
                    unsafe { addr.write(value.as_u64()) }
                }
                ty => unimplemented!("relocation type {:x} not supported", ty),
            }
        }

        Ok(())
    }
}

/// Check that the virtual offset belongs to a load segment.
fn check_is_in_load(elf_file: &ElfFile, virt_offset: u64) -> Result<(), &'static str> {
    for program_header in elf_file.program_iter() {
        if let Type::Load = program_header.get_type()? {
            if program_header.virtual_addr() <= virt_offset {
                let offset_in_segment = virt_offset - program_header.virtual_addr();
                if offset_in_segment < program_header.file_size() {
                    return Ok(());
                }
            }
        }
    }
    Err("offset is not in load segment")
}
