# BeskarOS Kernel

BeskarOS is a basic kernel for the x86_64 architecture.

## Architecture

The kernel is a monolithic x86_64 kernel written in pure Rust (with the exception of a single x86 ASM file for bootstrapping APs).
I am not planning on writing a Linux-like kernel, as I am mainly focusing on learning the basics.

## Features

- Arch
    - [X] x86_64
        - [x] Interrupts/GDT
        - [X] CPUID
        - [X] APIC
            - [x] LAPIC
            - [X] IOAPIC
        - [x] AP startup
        - [x] Randomness
        - [x] Systemcalls
    - [ ] aarch64
- Drivers
    - [ ] ACPI
        - [x] MADT
        - [x] HPET
        - [x] MCFG
        - [ ] FADT
    - [X] PCI/PCIe
        - [x] Devices
        - [X] MSI/MSI-X
    - [ ] USB
        - [ ] xHCI driver (partial)
        - [ ] USB 1/2/3 driver
        - [ ] Keyboard driver
    - [x] Time
        - [x] HPET
        - [x] x86_64
            - [x] TSC
            - [x] APIC Timer
    - [ ] NICs
        - [ ] Intel e1000e (partial)
    - Storage
        - [ ] AHCI
        - [ ] NVMe (partial)
- Memory
    - [x] Paging
    - [x] Physical/Virtual Allocators
    - [x] Address spaces / VMM
- Network
    - [ ] Network stack
        - [ ] ARP
        - [ ] ICMP
        - [ ] IPv4
        - [ ] UDP
        - [ ] TCP
    - [ ] Services
        - [ ] DHCP
        - [ ] DNS
        - [ ] Sockets
- Storage
    - [ ] GPT
    - [ ] FS
        - [ ] FAT32
        - [ ] ext2
    - [ ] VFS
- Processes
    - [x] Scheduling
    - [X] User space
    - [X] Binary loading
        - [X] ELF
- Video
    - [x] Character rendering
    - [x] Logging
    - [ ] GUI
    - [ ] GPU drivers (🤠)
