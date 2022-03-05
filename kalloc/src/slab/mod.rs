pub mod dynamic;
pub mod fixed;

pub trait SlabSize {}

pub struct SlabCheck<const N: usize>;
impl SlabSize for SlabCheck<128> {}
impl SlabSize for SlabCheck<256> {}
impl SlabSize for SlabCheck<512> {}
impl SlabSize for SlabCheck<1024> {}
impl SlabSize for SlabCheck<2048> {}
impl SlabSize for SlabCheck<4096> {}
