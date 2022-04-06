ENTRY(_start)

MEMORY {
    KERNEL_BUFFER   (rw)    : ORIGIN = 0x500, LENGTH = 512
    PAGE_TABLES     (rw)    : ORIGIN = 0x1000, LENGTH = 16K
    MEMORY_MAP      (rw)    : ORIGIN = 0x5000, LENGTH = 4K

    FIRST_STAGE     (rwx)   : ORIGIN = 0x7c00, LENGTH = 512

    BOOTLOADER      (rwx)   : ORIGIN = 0x7c00 + 512, LENGTH = 8M - 512
    KERNEL          (rwx)   : ORIGIN = 0x7c00 + 8M, LENGTH = 64M
}

SECTIONS {
    /* buffer for loading the kernel */
    .kernel_buffer : { _kernel_buffer = .; } > KERNEL_BUFFER
    
    /* bootloader page tables */
    .page_tables_ld : {
        __page_table_start = .;
        _p4 = . + 0x0000;
        _p3 = . + 0x1000;
        _p2 = . + 0x2000;
        _p1 = . + 0x3000;
        __page_table_end = . + 0x4000;
    } > PAGE_TABLES
    
    /* memory map buffer */
    .memory_map : { _memory_map = .; } > MEMORY_MAP
    
    /* bootloader first stage */
    .first_stage : { *(.boot-first-stage) } > FIRST_STAGE

    /* bootloader stage 2-4 and static kernel data */
    .bootloader : {
        _rest_of_bootloader_start_addr = .;
        *(.boot)
        *(.context_switch)
        *(.text .text.*)
        *(.rodata .rodata.*)
        *(.data .data.*)
        *(.bss .bss.*)
        *(.got)
        . = ALIGN(512);
        _rest_of_bootloader_end_addr = .;
    } > BOOTLOADER
    

    /* kernel */
    .kernel : { KEEP(*(.kernel)) } > KERNEL
}
