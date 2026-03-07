use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let srcd = PathBuf::from("DOOM");
    let mut c_files = Vec::new();

    for entry in std::fs::read_dir(&srcd)? {
        let entry = entry?;
        if let Some(filename) = entry.file_name().to_str()
            && filename.ends_with(".c")
        {
            let filepath = srcd.join(filename);
            println!("cargo::rerun-if-changed={}", filepath.display());
            c_files.push(filepath);
        }
    }

    cc::Build::new()
        .compiler("clang.exe")
        .files(&c_files)
        .target("x86_64-unknown-none")
        .flag("-ffreestanding")
        .flag("-nostdlib")
        .flag("-fno-builtin")
        .flag("-fPIC")
        .flag("-w")
        // compile without simd
        .flag("-mgeneral-regs-only")
        .flag("-flto")
        // make signed integer overflow wrap instead of UB
        .flag("-fwrapv")
        .compile("puredoom");

    Ok(())
}
