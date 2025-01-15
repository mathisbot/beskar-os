# BeskarOS Kernel

BeskarOS is a basic kernel for the x86-64 architecture.

## Architecture

The kernel is a monolithic x86-64 kernel written in pure Rust (with the exception of a single x86 ASM file for bootstrapping APs).
I am not planning on writing a kernel close to Linux, as I am focusing on learning the basics of x86.

## Features

- Boot
    - [ ] ACPI
        - [x] MADT
        - [x] HPET
        - [x] MCFG
        - [ ] FADT
- CPU
    - [x] Interrupts/GDT
    - [ ] APIC
        - [x] LAPIC
        - [ ] IOAPIC
    - [x] AP startup
    - [x] Randomness
    - [x] Time
    - [ ] Systemcalls
- Memory
    - [x] Paging
    - [x] Physical/Virtual Allocators
    - [ ] Address spaces (partial)
- Filesystem
    - [ ] AHCI driver
    - [ ] NVMe driver
    - [ ] GPT
    - [ ] ext2/ext4
- Drivers
    - [ ] PCI/PCIe
        - [x] Devices
        - [ ] MSI/MSI-X (partial)
    - [ ] USB
        - [ ] xHCI drive
        - [ ] USB 1/2/3 driver
        - [ ] Keyboard driver
- Network
    - [ ] Network stack
        - [ ] NIC drivers
            - [ ] Intel e1000e
        - [ ] ARP
        - [ ] ICMP
        - [ ] IPv4
        - [ ] UDP
        - [ ] TCP
    - [ ] Services
        - [ ] DHCP
        - [ ] DNS
        - [ ] Sockets
- Processes
    - [x] Scheduling
    - [ ] User space
    - [ ] ELF loading
- Video
    - [x] Character rendering
    - [x] Logging
    - [ ] GUI
    - [ ] GPU driver (🤠)
