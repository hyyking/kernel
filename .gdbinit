tui enable
directory bootloader
set print asm-demangle on
set print pretty on
file target/x86_64-bootloader/release/bios
file target/target/debug/kernel
target remote :1234

break bootloader_main
break switch_to_kernel
break kmain
c
