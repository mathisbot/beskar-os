//! When it comes to building multiple binaries in a single workspace,
//! we are given two choices:
//!
//! - Use the main binary (i.e. `src`) as a CLI application to interact with the
//!   workspace. This as the disadvantage of being longer to type as well as
//!   longer to build as it is harder to benefit from parallel compilation.
//! - Use a build script to build the binaries, using the `bindeps` unstable
//!   cargo feature. This feature received a lot of hype so it is likely to
//!   become stable in the future. This has the disadvantage of being less
//!   flexible, as a simple `cargo check` will run the build script.
//!
//! The second approach is the one we are taking here as it is extremely convenient
//! to iterate on the workspaece using only `cargo b`.
#![forbid(unsafe_code)]
use std::{env::var, fs};

/// List of package names for userspace applications.
const USERSPACE_APPS: [&str; 1] = ["bashkar"];

/// A macro to print cargo instructions.
macro_rules! cargo {
    ($param:expr, $value:expr) => {
        println!("cargo:{param}={value}", param = $param, value = $value);
    };
}

/// Converts a crate name to the corresponding CARGO_BIN_FILE environment variable name.
fn crate_name_to_cargo_venv(crate_name: &str) -> String {
    format!(
        "CARGO_BIN_FILE_{}",
        crate_name
            .chars()
            .map(|c| match c {
                'a'..='z' | 'A'..='Z' | '0'..='9' => c.to_ascii_uppercase(),
                '-' | '_' => '_',
                _ => panic!("Invalid character in crate name: '{c}'"),
            })
            .collect::<String>()
    )
}

fn main() {
    cargo!("rerun-if-changed", "./build.rs");
    cargo!("rerun-if-changed", "./beskar-core");
    cargo!("rerun-if-changed", "./beskar-hal");
    cargo!("rerun-if-changed", "./beskar-lib");
    cargo!("rerun-if-changed", "./bootloader");
    cargo!("rerun-if-changed", "./hyperdrive");
    cargo!("rerun-if-changed", "./kernel");
    cargo!("rerun-if-changed", "./userspace");

    let bootloader_path = var("CARGO_BIN_FILE_BOOTLOADER").unwrap();
    let kernel_path = var("CARGO_BIN_FILE_KERNEL").unwrap();

    fs::create_dir_all("efi_disk/efi/boot").expect("Failed to create efi_disk/efi/boot directory");

    // Copy the bootloader and kernel binaries to the efi_disk directory
    fs::copy(&bootloader_path, "efi_disk/efi/boot/bootx64.efi")
        .expect("Failed to copy bootloader.efi");
    fs::copy(&kernel_path, "efi_disk/efi/kernelx64.elf").expect("Failed to copy kernel");

    // TODO:: Build a disk image for the ramdisk
    let bashkar = var("CARGO_BIN_FILE_BASHKAR").unwrap();
    fs::copy(&bashkar, "efi_disk/efi/ramdisk.img").expect("Failed to copy bashkar");
    for crate_name in USERSPACE_APPS {
        let cargo_venv = crate_name_to_cargo_venv(crate_name);
        let _built_path = var(cargo_venv).expect("Failed to get built path");
        // TODO: Add to the ramdisk image
    }
}
