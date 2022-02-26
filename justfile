KERNELIMG := "target/kernel.img"
export KERNEL_MANIFEST := `find $(pwd -P)/kernel -type f -name Cargo.toml`

QEMU_ARGS := "-enable-kvm -cpu host -drive format=raw,file=" + KERNELIMG

run: image 
    qemu-system-x86_64 {{QEMU_ARGS}} -serial stdio

run-debug: image 
    qemu-system-x86_64 {{QEMU_ARGS}} -d int,cpu_reset -no-reboot -serial stdio

run-gdb: image
    qemu-system-x86_64 {{QEMU_ARGS}} -d int,cpu_reset -no-reboot -s -S -nographic

image: kernel bootloader
    #!/usr/bin/sh
    BOOTLOADER=$(find target/x86_64-bootloader/ -type f -name bios)
    
    objcopy -I "elf64-x86-64" -O "binary" $BOOTLOADER {{KERNELIMG}}
    
    BLOCKS=$(du -B512 {{KERNELIMG}} | rg -o "\d+")
    fallocate -l $(expr 512 \* $BLOCKS) {{KERNELIMG}}
    printf "\e[32;1m[3/3] Created image\n\e[0m"

@kernel:
    cd kernel && cargo build
    printf "\e[32;1m[1/3] Kernel build successful\n\e[0m"

@bootloader $KERNEL=`find ~+ -type f -name kernel` $RUSTFLAGS="-C opt-level=s":
    cd bootloader && cargo build --bin bios --release --features bios_bin 
    printf "\e[32;1m[2/3] Bootloader build successful\n\e[0m"
