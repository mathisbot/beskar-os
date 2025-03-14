use std::{env::var, fs};

fn main() {
    println!("cargo:rerun-if-changed=./beskar-core");
    println!("cargo:rerun-if-changed=./beskar-lib");
    println!("cargo:rerun-if-changed=./bootloader");
    println!("cargo:rerun-if-changed=./hyperdrive");
    println!("cargo:rerun-if-changed=./kernel");
    println!("cargo:rerun-if-changed=./userspace");

    let bootloader_path = var("CARGO_BIN_FILE_BOOTLOADER").unwrap();
    let kernel_path = var("CARGO_BIN_FILE_KERNEL").unwrap();
    let hello_world = var("CARGO_BIN_FILE_HELLO_WORLD").unwrap();

    fs::create_dir_all("efi_disk/efi/boot").expect("Failed to create efi_disk/efi/boot directory");

    // Copy the bootloader and kernel binaries to the efi_disk directory
    fs::copy(&bootloader_path, "efi_disk/efi/boot/bootx64.efi")
        .expect("Failed to copy bootloader.efi");
    fs::copy(&kernel_path, "efi_disk/efi/kernelx64.elf").expect("Failed to copy kernel");

    // TODO:: Build a disk image for the ramdisk
    fs::copy(&hello_world, "efi_disk/efi/ramdisk.img").expect("Failed to copy userspace");
}
