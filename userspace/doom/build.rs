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

    let compiler = if cfg!(windows) { "clang.exe" } else { "gcc" };

    cc::Build::new()
        .compiler(compiler)
        .std("gnu17")
        .files(&c_files)
        .flag("-ffreestanding")
        .flag("-nostdlib")
        .flag("-fno-builtin")
        .flag("-fno-stack-protector")
        .flag("-fPIC")
        .flag("-w")
        .flag("-march=x86-64-v2")
        .flag("-mfpmath=sse")
        .flag("-mno-avx")
        .flag("-mno-avx2")
        .flag("-mno-fma")
        .flag("-mno-f16c")
        .flag("-mno-mmx")
        // make signed integer overflow wrap instead of UB
        .flag("-fwrapv")
        .compile("puredoom");

    Ok(())
}
