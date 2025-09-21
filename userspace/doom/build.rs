use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let srcd = PathBuf::from("DOOM");
    let mut c_files = Vec::new();

    for entry in std::fs::read_dir(&srcd)? {
        let entry = entry?;
        if let Some(filename) = entry.file_name().to_str()
            && filename.ends_with(".c")
        {
            println!("cargo::rerun-if-changed={}", filename);
            c_files.push(srcd.join(filename));
        }
    }

    cc::Build::new()
        .compiler("clang.exe")
        .files(&c_files)
        .target("x86_64-unknown-none")
        .flag("-ffreestanding")
        .flag("-nostdlib")
        .flag("-fno-builtin")
        .flag("-fno-stack-protector")
        .flag("-mno-red-zone")
        .flag("-fPIC")
        // suppress warnings from clang
        .flag("-w")
        // compile without simd
        .flag("-mgeneral-regs-only")
        .compile("puredoom");

    Ok(())
}
