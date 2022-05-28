tui enable
set print asm-demangle on
set print pretty on

directory bootloader
directory kernel
directory libx64
directory drivers/page_mapper


# file target/target/debug/kernel
file target/x86_64-bootloader/release/bios

target remote :1234

hbreak _start
