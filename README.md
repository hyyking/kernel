# Kernel

Dependencies:

- [`QEMU`](https://www.qemu.org/)
- [`Cargo`](https://doc.rust-lang.org/cargo/)
- [`just`](https://github.com/casey/just)

Build:

- `just run`:   build the kernel and run with qemu
- `just image`: build the kernel and create an image
- `just build`: build the kernel

## Goal

Build a micro/exokernel

## Features

- [X] IDT/GDT
- [X] Paging
- [X] Logging
- [ ] Allocator
- [ ] Scheduler
- [ ] Filesystem
- [ ] Network
