export KERNEL_MANIFEST := `find $(pwd -P)/kernel -type f -name Cargo.toml`

KERNELIMG := "target/kernel.img"
QEMU_ARGS := "-enable-kvm -cpu host -drive format=raw,file=" + KERNELIMG
SERIAL_ADDR := "127.0.0.1:8000"

#cargo r --bin konsole &
#sleep 0.5
run: konsole image
    cargo run --release --bin konsole -- {{SERIAL_ADDR}} &
    sleep 0.5
    qemu-system-x86_64 {{QEMU_ARGS}} -serial tcp:{{SERIAL_ADDR}}
#nc -l 8000 &

@konsole:
    cargo build --release --bin konsole

run-debug: image 
    qemu-system-x86_64 {{QEMU_ARGS}} -d int,cpu_reset -no-reboot -serial stdio

run-gdb: image
    qemu-system-x86_64 {{QEMU_ARGS}} -d int,cpu_reset -no-reboot -s -S -serial stdio

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

bootloader $KERNEL=`find ~+ -type f -name kernel`:
    #!/usr/bin/zsh
    KERNEL_TEXT_SIZE=$(llvm-size $KERNEL | awk '(NR == 2) {print $1}')
    
    echo $KERNEl
    if [[ $KERNEL_TEXT_SIZE -eq 0 ]] then
        echo "Kernel has 0 text size, please define an entry point"
        exit 1
    fi

    cd bootloader && cargo build --bin bios --release --features bios_bin
    printf "\e[32;1m[2/3] Bootloader build successful\n\e[0m"

bootloader-doc $KERNEL=`find ~+ -type f -name kernel`:
    cd bootloader && cargo doc --bin bios --features bios_bin --open

