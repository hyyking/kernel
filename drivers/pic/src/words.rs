use bitfield::bitfield;

bitfield! {
    /// # Initialization Control Word 1 (ICW1)
    ///
    /// ## Format
    ///
    /// Bit Number | Value | Description
    /// -----------|-------|-----------------------------------------------------------------
    /// 0          | IC4   | If set(1), the PIC expects to recieve IC4 during initialization.
    /// 1          | SNGL  | 1: Single PIC; 0: PIC is cascaded, PICs and ICW3 must be sent.
    /// 2          | ADI   | 1: CALL address interval is 4, else 8 (ignored in x86).
    /// 3          | LTIM  | 1: Level Triggered Mode; 0: Edge Triggered Mode.
    /// 4          | 1	   | 1: PIC is to be initialized
    /// 5          | 0	   | x86: zero; MCS-80/85: Vector Address
    /// 6          | 0	   | x86: zero; MCS-80/85: Vector Address
    /// 7          | 0	   | x86: zero; MCS-80/85: Vector Address
    #[derive(Clone, Copy)]
    #[repr(transparent)]
    pub unsafe struct ICW1: u8 {
        pub ic4: 0..1,
        pub sngl: 1..2,
        pub adi: 2..3,
        pub ltim: 3..4,
        pub init: 4..5,
        zero1: 5..6,
        zero2: 6..7,
        zero3: 7..8,
    }
}

bitfield! {
    /// # Initialization Control Word 2 (ICW2)
    ///
    /// ## Format
    ///
    /// In 80x86 mode, specifies the interrupt vector address. May be set to 0 in x86 mode.
    ///
    /// Bit Number | Value         | Description
    /// -----------|---------------|----------------------------------------------------
    /// 0-2	   | A8/A9/A10     | Address bits A8-A10 for IVT when in MCS-80/85 mode.
    /// 3-7	   | A11-15(T3-7)) | Address bits A11-A15 for IVT when in MCS-80/85.
    ///
    #[derive(Clone, Copy)]
    #[repr(transparent)]
    pub unsafe struct ICW2: u8 {
        pub a8a10: 0..3,
        pub a11a15: 3..8,
    }
}

/// # Initialization Control Word 3 (ICW3)
///
/// ## Primary PIC Format
///
/// Bit Number | Value | Description
/// -----------|-------|-----------------------------------------------------------------
/// 0-7        | S0-S7 | Specifies what Interrupt Request (IRQ) is connected to slave PIC
///
/// ## Secondary PIC Format
///
/// Bit Number | Value | Description
/// -----------|-------|------------------------------------------------------------------
/// 0-2        | ID0   | IRQ number the master PIC uses to connect to (In binary notation)
/// 3-7        | 0     | Reserved, must be 0
///
#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct ICW3(pub u8);

bitfield! {
    /// # Initialization Control Word 4 (ICW4)
    ///
    /// ## Format
    ///
    /// Bit Number | Value | Description
    /// -----------|-------|-----------------------------------------------------------
    /// 0	   | uPM   | 1: 80x86 mode; 0: MCS-80/86 mode
    /// 1	   | AEOI  | On the last interrupt acknowledge pulse, controller EOIs
    /// 2	   | M/S   | Only use if BUF is set. 1: buffer master; 0: buffer slave.
    /// 3	   | BUF   | 1: controller operates in buffered mode
    /// 4	   | SFNM  | Special Fully Nested Mode. (Large PIC cascades)
    /// 5-7	   | 0	   | Reserved, must be 0
    ///
    #[derive(Clone, Copy)]
    #[repr(transparent)]
    pub unsafe struct ICW4: u8 {
        pub x86mode: 0..1,
        pub aeoi: 1..2,
        pub ms: 2..3,
        pub buf: 3..4,
        pub sfnm: 4..5,
        zero: 5..8,
    }
}
