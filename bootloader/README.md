# BeskarOS Bootloader

BeskarOS bootloader is a basic UEFI 2 bootloader.

It loads the kernel, sets up a nice environment and jumps to the kernel.

## Architecture

The bootloader is a UEFI application.

It has few features :
- [x] Kernel ELF loading
- [ ] Arch
    - [x] x86_64
        - [x] Setup paging
        - [x] Early memory mapping
    - [ ] aarch64
- [x] Gather information about SMP
- [x] Screen
    - [X] Initialization
    - [X] Logging
- [x] Handle ACPI
- [X] Ramdisk
