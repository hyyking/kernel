use libx64::{
    address::VirtualAddr,
    paging::{
        entry::Flags,
        frame::{FrameAllocator, FrameError, FrameRange, FrameTranslator, PhysicalFrame},
        page::{Page, PageMapper, PageRange, PageTranslator, TlbFlush, TlbMethod},
        table::{Level4, PageLevel, Translation},
        Page4Kb, PageCheck, PageSize, PinTableMut,
    },
};

#[repr(transparent)]
pub struct TracingMapper<M>(M);

impl<M, const N: usize> PageMapper<N> for TracingMapper<M>
where
    PageCheck<N>: PageSize,
    M: PageMapper<N>,
{
    unsafe fn from_level4(page: PinTableMut<'_, Level4>) -> Self {
        Self(M::from_level4(page))
    }

    fn level4(&mut self) -> PinTableMut<'_, Level4> {
        self.0.level4()
    }

    fn map<A>(
        &mut self,
        page: Page<N>,
        frame: PhysicalFrame<N>,
        flags: Flags,
        allocator: &mut A,
    ) -> Result<TlbFlush<N>, FrameError>
    where
        A: FrameAllocator<Page4Kb>,
    {
        trace!("Mapping page: {:?} -> {:?}", &page, &frame);
        self.0.map(page, frame, flags, allocator)
    }

    fn update_flags(&mut self, page: Page<N>, flags: Flags) -> Result<TlbFlush<N>, FrameError> {
        self.0.update_flags(page, flags)
    }

    fn unmap(&mut self, page: Page<N>) -> Result<TlbFlush<N>, FrameError> {
        self.0.unmap(page)
    }

    fn id_map<A>(
        &mut self,
        frame: PhysicalFrame<N>,
        flags: Flags,
        allocator: &mut A,
    ) -> Result<TlbFlush<N>, FrameError>
    where
        A: FrameAllocator<Page4Kb>,
    {
        self.0.id_map(frame, flags, allocator)
    }

    #[tracing::instrument(skip(self, allocator, flags, method), target = "map_range", fields(flags = flags.bits()))]
    fn map_range<A>(
        &mut self,
        pages: PageRange<N>,
        frames: FrameRange<N>,
        flags: Flags,
        allocator: &mut A,
        method: TlbMethod,
    ) -> Result<(), FrameError>
    where
        A: FrameAllocator<Page4Kb>,
    {
        self.0.map_range(pages, frames, flags, allocator, method)
    }

    #[tracing::instrument(skip(self, allocator, flags, method), target = "map_range_alloc", fields(flags = flags.bits()))]
    fn map_range_alloc<A>(
        &mut self,
        pages: PageRange<N>,
        flags: Flags,
        allocator: &mut A,
        method: TlbMethod,
    ) -> Result<(), FrameError>
    where
        A: FrameAllocator<Page4Kb> + FrameAllocator<N>,
    {
        self.0.map_range_alloc(pages, flags, allocator, method)
    }

    #[tracing::instrument(skip(self, allocator, flags, method), target = "id_map_range", fields(flags = flags.bits()))]
    fn id_map_range<A>(
        &mut self,
        frames: FrameRange<N>,
        flags: Flags,
        allocator: &mut A,
        method: TlbMethod,
    ) -> Result<(), FrameError>
    where
        A: FrameAllocator<Page4Kb>,
    {
        self.0.id_map_range(frames, flags, allocator, method)
    }
}

impl<M> PageTranslator for TracingMapper<M>
where
    M: PageTranslator,
{
    fn try_translate(&mut self, addr: VirtualAddr) -> Result<Translation, FrameError> {
        self.0.try_translate(addr)
    }
}

impl<M> FrameTranslator<(), Page4Kb> for TracingMapper<M>
where
    M: FrameTranslator<(), Page4Kb>,
{
    #[inline]
    unsafe fn translate_frame<'a>(
        &self,
        frame: PhysicalFrame<Page4Kb>,
    ) -> PinTableMut<'a, <() as PageLevel>::Next> {
        self.0.translate_frame(frame)
    }
}

impl<M> From<M> for TracingMapper<M> {
    fn from(mapper: M) -> Self {
        Self(mapper)
    }
}
