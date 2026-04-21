cfg_select! {
    target_arch = "aarch64" => {
        mod aarch64;
        pub use aarch64::*;
    }
    target_arch = "x86_64" => {
        mod x86_64;
        pub use x86_64::*;
    }
    _ => {
        compile_error!("Unsupported architecture");
    }
}
