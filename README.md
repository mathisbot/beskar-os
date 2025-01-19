# BeskarOS

This repository is a very basic x86-64 hobby OS written in pure Rust that boots on UEFI 2.0.
It is named after the alloy used to forge Mandalorian armors, which cannot rust.

## Design and architecture

- Bootloader: Custom UEFI 2 bootloader
- Kernel: Modern x86-64 kernel with SMP support
- Hyperdrive: Bare metal utility module for the OS

You will find more information in their respective READMEs, especially in `kernel/README.md` for the features.

## Usage

You easily use BeskarOS, however keep in mind that it is currently in an early stage where it only initializes and prints information.

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
& "qemu-system-x86_64.exe" -drive if=pflash,format=raw,readonly=on,file=<x86_64-OVMF> -drive format=raw,file=fat:rw:efi_disk -smp <NB_CORES> -m <RAM_SIZE> -cpu <CPU_ARCH> -accel <ACCEL_BACKEND> -serial stdio -device qemu-xhci -device usb-kbd -M q35
```

Where:
- `<x86_64-OVMF>` is the file `<QEMU>/share/edk2-x86_64-code.fd`. Please note that the OVMF file only comes preinstalled on Windows versions of QEMU. You will have to download them on Linux. You will find many tutorials online. It is currently the only way to allow QEMU to use UEFI.
- `<NB_CORES>` must be 1 or more, but setting it to at least 2 is better.
- `<RAM_SIZE>` is in MiB. It must be at least 64.
- `<CPU_ARCH>` specifies the CPU architecture to emulate. QEMU's default amd64 CPU doesn't support some features that are mandatory such as `FSGSBASE`. A good choice is `max` (Windows) or `host` (Linux), or even `qemu64,+pse,+msr,+apic,+fsgsbase,+rdrand` for minimal features.
- `<ACCEL_BACKEND>` allows QEMU to use acceleration based on your OS. On Linux, you can set it to `kvm`, and to `whpx` on Windows (currently incompatible with OMVF files).

Other useful parameters:
- `-device virtio-vga -display <BACKEND>,gl=on`: If having a fixed 2560x1600 resolution bothers you, you can use a better-fitting framebuffer with these options. Replace `<BACKEND>` with either `sdl` or `gtk`.
- If you're not interested in graphics (for debugging purposes) replace `-serial stdio` with `-nographics`.

#### Troubleshooting

If the firmware starts and boots on the UEFI shell instead of the bootloader, try deleting `efi_disk/NvVars`.

If you are using `-accel whpx` and QEMU boots with a blank window right before crashing, remove WHPX acceleration. This a "bug" (apparently, they are not planning on fixing Windows builds of QEMU).

### Running on baremetal

If you want to run the OS on a real baremetal server, make sure that you have a proper x86-64 server that supports UEFI 2 and has at least 64 MiB of RAM.
Secure boot must also be disabled.

Copy the content of the directory `efi_disk` to a FAT32 filesystem on a GPT or MBR partition of a drive, and you're good to go!

#### Troubleshooting

I don't have access to a nice testing hardware. If you find anything, please contact me!

If the bootloader runs but the computer stalls right after (i.e. the screen gets filled with text then turns black), the kernel has likely crashed with an INVALID OPCODE exception very early. Try to build the kernel for an older version of x86-64. If you happen to have a COM1 serial port on your hardware, you can receive debug messages when connecting to it (using PuTTY for example).

In any other cases, the kernel **SHOULD** print information on the screen about any problem it encounters.

## Sources and inspirations

My warmest thanks to all the [OSDev](https://wiki.osdev.org/) contributors, without whom it would have been impossible to acquire all the information needed to write such code.

Special thanks to Philipp Oppermann, for his [BlogOS ed.3](https://github.com/phil-opp/blog_os) series and for his [bootloader](https://github.com/rust-osdev/bootloader) crate, which enabled me to start from scratch with clear, easy-to-understand explanations.
