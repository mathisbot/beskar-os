use std::{env::var, fs};

fn main() {
    let bootloader_path = var("CARGO_BIN_FILE_BOOTLOADER").unwrap();
    let kernel_path = var("CARGO_BIN_FILE_KERNEL").unwrap();

    fs::create_dir_all("efi_disk/efi/boot").expect("Failed to create efi_disk/efi/boot directory");

    // Copy the bootloader and kernel binaries to the efi_disk directory
    fs::copy(&bootloader_path, "efi_disk/efi/boot/bootx64.efi")
        .expect("Failed to copy bootloader.efi");
    fs::copy(&kernel_path, "efi_disk/efi/kernelx64.elf").expect("Failed to copy kernel");

    println!("cargo:rerun-if-changed=./bootloader");
    println!("cargo:rerun-if-changed=./hyperdrive");
    println!("cargo:rerun-if-changed=./kernel");
}
