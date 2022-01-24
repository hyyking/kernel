pub mod idt {
    use libx64::descriptors::interrupt::IstIndex;

    #[repr(u8)]
    pub enum IstEntry {
        DoubleFault = 0,
    }

    impl From<IstEntry> for IstIndex {
        fn from(val: IstEntry) -> Self {
            match val {
                IstEntry::DoubleFault => IstIndex::Idx1,
            }
        }
    }
    impl From<IstEntry> for usize {
        fn from(e: IstEntry) -> Self {
            usize::from(e as u8)
        }
    }
}

pub mod gdt {
    use libx64::segments::SegmentSelector;

    #[derive(Debug)]
    pub struct Selectors {
        pub code_segment: SegmentSelector,
        pub task_state: SegmentSelector,
    }
}
