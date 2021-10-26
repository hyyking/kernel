#![allow(non_upper_case_globals)]

pub mod bits {
    pub const Kb: u64 = 1024;
    pub const Mb: u64 = Kb * 1024;
    pub const Gb: u64 = Mb * 1024;
}

pub mod bytes {
    pub const Kb: u64 = 1024 / 8;
    pub const Mb: u64 = Kb * 1024;
    pub const Gb: u64 = Mb * 1024;
}
