#![forbid(unsafe_code)]
use std::{env::var, fs, str::FromStr};

const USERSPACE_APPS: [&str; 1] = ["hello_world"];

fn crate_name_to_cargo_venv(crate_name: &str) -> String {
    let mut cargo_venv = String::from_str("CARGO_BIN_FILE_").unwrap();
    for c in crate_name.chars() {
        if c.is_ascii_alphanumeric() {
            cargo_venv.push(c.to_ascii_uppercase());
        } else if c == '-' || c == '_' {
            cargo_venv.push('_');
        } else {
            panic!("Invalid character in crate name: {}", c);
        }
    }
    cargo_venv
}

fn main() {
    println!("cargo:rerun-if-changed=./beskar-core");
    println!("cargo:rerun-if-changed=./beskar-lib");
    println!("cargo:rerun-if-changed=./bootloader");
    println!("cargo:rerun-if-changed=./hyperdrive");
    println!("cargo:rerun-if-changed=./kernel");
    println!("cargo:rerun-if-changed=./userspace");

    let bootloader_path = var("CARGO_BIN_FILE_BOOTLOADER").unwrap();
    let kernel_path = var("CARGO_BIN_FILE_KERNEL").unwrap();

    fs::create_dir_all("efi_disk/efi/boot").expect("Failed to create efi_disk/efi/boot directory");

    // Copy the bootloader and kernel binaries to the efi_disk directory
    fs::copy(&bootloader_path, "efi_disk/efi/boot/bootx64.efi")
        .expect("Failed to copy bootloader.efi");
    fs::copy(&kernel_path, "efi_disk/efi/kernelx64.elf").expect("Failed to copy kernel");

    // TODO:: Build a disk image for the ramdisk
    let hello_world = var("CARGO_BIN_FILE_HELLO_WORLD").unwrap();
    fs::copy(&hello_world, "efi_disk/efi/ramdisk.img").expect("Failed to copy userspace");
    for crate_name in USERSPACE_APPS {
        let cargo_venv = crate_name_to_cargo_venv(crate_name);
        let _built_path = var(cargo_venv).expect("Failed to get built path");
        // TODO: Add to the ramdisk image
    }
}
