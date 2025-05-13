# BeskarOS Kernel

BeskarOS is a basic kernel for the x86_64 architecture.

## Architecture

The kernel is a monolithic x86_64 kernel written in pure Rust (with the exception of a single x86 ASM file for bootstrapping APs).
I am not planning on writing a Linux-like kernel, as I am mainly focusing on learning the basics.

## Features

- Arch
    - [ ] aarch64
    - [X] x86_64
        - [x] AP startup
        - [X] APIC
            - [x] LAPIC
            - [X] IOAPIC
        - [X] CPUID
        - [x] GDT/TSS
        - [x] Interrupts
        - [x] Randomness
        - [x] Systemcalls
- Drivers
    - [ ] ACPI
        - [ ] DSDT (partial)
            - [ ] AML
        - [ ] FADT (partial)
        - [x] HPET
        - [x] MADT
        - [x] MCFG
        - [x] RSDT/XSDT
    - NIC
        - [ ] Intel e1000e (partial)
    - [X] PCI
        - [X] PCI/PCI-X
            - [X] MSI
        - [X] PCIe
            - [X] MSI-X
        - [x] Devices
    - [ ] PS/2
        - [X] Keyboard
        - [ ] Mouse
    - Storage
        - [ ] AHCI
        - [ ] NVMe (partial)
    - [x] Time
        - [x] HPET
        - x86_64
            - [x] APIC Timer
            - [x] TSC
    - USB
        - [ ] Controllers
            - [ ] EHCI
            - [ ] xHCI (partial)
        - [ ] USB
            - [ ] USB 2
            - [ ] USB 3
        - Devices
            - [ ] Generic Keyboard
- Memory
    - [x] Paging
    - [x] Physical/Virtual Allocators
    - [x] Address spaces / VMM
- Network
    - [ ] Network stack
        - L2
            - [X] Ethernet
        - L3
            - [X] ARP
            - [ ] IP
                - [ ] IPv4
                - [ ] IPv6
        - L4
            - [ ] ICMP
            - [ ] UDP
            - [ ] TCP
    - [ ] Services
        - [ ] DHCP
        - [ ] DNS
        - [ ] Sockets
- Processes
    - [x] Scheduling
        - [X] Context save/switch
        - [X] Priority handling
        - [ ] Sleeping threads (partial)
        - [X] TLS
    - [X] User space
    - [X] Binary loading
        - [X] ELF
- Storage
    - [ ] Partitions
        - [ ] MBR
        - [ ] GPT
    - [ ] FS
        - [X] Device files
        - [X] FAT12/16/32
        - [ ] ext2
        - [ ] ext4
    - [X] VFS
- Video
    - [x] Character rendering
    - [x] Logging
    - [ ] GUI
    - [ ] GPU drivers (🤠)
