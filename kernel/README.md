# BeskarOS Kernel

BeskarOS is a basic kernel for the x86_64 architecture.

## Architecture

The kernel is a monolithic x86_64 kernel written in pure Rust (with the exception of a single x86 ASM file for bootstrapping APs).
I am not planning on writing a Linux-like kernel, as I am mainly focusing on learning the basics.

## Features

- Arch
    - [ ] x86_64
        - [x] Interrupts/GDT
        - [ ] APIC
            - [x] LAPIC
            - [ ] IOAPIC
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
    - [ ] PCI/PCIe
        - [x] Devices
        - [ ] MSI/MSI-X (partial)
    - [ ] USB
        - [ ] xHCI driver
        - [ ] USB 1/2/3 driver
        - [ ] Keyboard driver
    - [x] Time
        - [x] HPET
        - [x] x86_64
            - [x] TSC
            - [x] APIC Timer
    - Network
        - [ ] NICs
            - [ ] Intel e1000e
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
        - [ ] AHCI
        - [ ] NVMe
- Memory
    - [x] Paging
    - [x] Physical/Virtual Allocators
    - [ ] Address spaces (partial)
- FS
    - [ ] GPT
    - [ ] ext2/ext4
- Processes
    - [x] Scheduling
    - [ ] User space
    - [ ] ELF loading
- Video
    - [x] Character rendering
    - [x] Logging
    - [ ] GUI
    - [ ] GPU drivers (🤠)
