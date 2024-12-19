# BeskarOS

This repository is a very basic 64-bit hobby OS written in pure Rust that boots on UEFI 2.0.
It is named after the alloy used to forge Mandalorian armors, which cannot rust.

## Requirements

- Rust (nightly)

Because of `rust-toolchain.toml`, `cargo` will automatically download the nightly channel if it is not already installed on you system.

## Usage

### Building

The OS can be built using `cargo build --release`.
The result of the building process lies inside the `efi_disk` directory.

You can optimize the built files for your specific target CPU. To do so, modify this line in `.cargo/cargo.toml` :

```toml
[build]
rustflags = ["-C", "target-cpu=<CPU>"]
```

Modify `<CPU>` with any of the CPU listed by `rustc --print target-cpus`.
A great choice for most of modern CPUs is `x86-64-v3` (`v4` if it is very modern), but it is best to set it to the exact model you have!

### Running on QEMU

If you want to run the OS on a testing virtual machine on QEMU, you can do so by running the following command :

```powershell
& "qemu-system-x86_64.exe" -drive if=pflash,format=raw,readonly=on,file=<x86_64-OVMF> -drive format=raw,file=fat:rw:efi_disk -smp <NB_CORES> -m <RAM_SIZE> -cpu <CPU_ARCH> -accel <ACCEL_BACKEND> -serial stdio -device qemu-xhci,id=xhci
```

Where:
- `<x86_64-OVMF>` is the file `<QEMU>/share/edk2-x86_64-code.fd`. Please note that the OVMF file only comes preinstalled on Windows versions of QEMU. You will have to download them on Linux. You will find many tutorials online. It is currently the only way to allow QEMU to use UEFI.
- `<NB_CORES>` must be 1 or more, but setting it to at least 2 is better.
- `<RAM_SIZE>` is in MiB. It must be at least 64.
- `<CPU_ARCH>` specifies the CPU architecture to emulate. QEMU's default amd64 CPU doesn't support some features that are mandatory such as `FSGSBASE`. A good choice is `max` (Windows) or `host` (Linux).
- `<ACCEL_BACKEND>` allows QEMU to use acceleration based on your OS. On Linux, you can set it to `kvm`, and to `whpx` on Windows (currently incompatible with OMVF files).

Standard output will be filled with early logging info, such as memory initialization, which is needed to initialize windows (thus screen logging).

### Running on baremetal

If you want to run the OS on a real baremetal server, make sure that you have a proper x86-64 server that supports UEFI 2, SSE2 and has at least 64 MiB of RAM.
Secure boot must also be disabled.

Copy the content of the directory `efi_disk` to a FAT32 filesystem on a GPT or MBR partition of a drive, and you're good to go!

## Architecture

This repository is a mono-repo. You will find two major components :

- Bootloader
- Kernel

Please refer to their READMEs for more information.

## TO-DOs

- APIC [WIP]
- HPET [WIP]
- Process [WIP]
- User mode
- Storage
- USB
- FADT
- Optimizations
- Stabilization
- Graphics

## Sources and inspirations

My warmest thanks to all the [OSDev](https://wiki.osdev.org/) contributors, without whom it would have been impossible to acquire all the information needed to write such code.

Special thanks to Philipp Oppermann, for his [BlogOS ed.3](https://github.com/phil-opp/blog_os) series and for his [bootloader](https://github.com/rust-osdev/bootloader) crate, which enabled me to start from scratch with clear, easy-to-understand explanations.
