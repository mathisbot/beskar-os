# BeskarOS Bootloader

BeskarOS bootloader is a basic UEFI 2 bootloader.

It loads the kernel, sets up a nice environment and jumps to the kernel.

## Architecture

The bootloader is a UEFI application.

It has few features :
- [x] Gather information about SMP
- [x] Initialize screen
- [x] Handle ACPI
- [ ] Arch
    - [x] x86_64
        - [x] Setup paging
        - [x] Early memory mapping
    - [ ] aarch64
- [ ] Initial Ramdisk?
- [x] Kernel ELF loading
