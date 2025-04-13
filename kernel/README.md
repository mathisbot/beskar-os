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
        - [ ] FADT
        - [x] HPET
        - [x] MADT
        - [x] MCFG
    - [ ] NICs
        - [ ] Intel e1000e (partial)
    - [X] PCI/PCIe
        - [x] Devices
        - [X] MSI/MSI-X
    - [ ] PS/2
        - [X] Keyboard
        - [ ] Mouse
    - Storage
        - [ ] AHCI
        - [ ] NVMe (partial)
    - [x] Time
        - [x] HPET
        - [x] x86_64
            - [x] TSC
            - [x] APIC Timer
    - [ ] USB
        - [ ] Controllers
            - [ ] xHCI (partial)
            - [ ] EHCI
        - USB
            - [ ] USB 3
            - [ ] USB 2
        - Devices
            - [ ] Generic Keyboard
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
