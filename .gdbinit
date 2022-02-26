tui enable
set print asm-demangle on
set print pretty on

directory bootloader
directory kernel

symbol-file target/x86_64-bootloader/release/bios
file target/target/debug/kernel

target remote :1234

hbreak switch_to_kernel
hbreak _start
