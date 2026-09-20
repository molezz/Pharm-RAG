fn main() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();

    // Compile C23 compat symbols when targeting Linux GNU (e.g. Ubuntu 20.04/22.04 with glibc < 2.38)
    if target_os == "linux" && target_env == "gnu" {
        cc::Build::new()
            .file("c_src/c23_compat.c")
            .compile("isoc23_compat");

        // Force linker to pull symbols from isoc23_compat static archive
        println!("cargo:rustc-link-arg=-Wl,--undefined=__isoc23_strtoull");
        println!("cargo:rustc-link-arg=-Wl,--undefined=__isoc23_strtoul");
        println!("cargo:rustc-link-arg=-Wl,--undefined=__isoc23_strtoll");
        println!("cargo:rustc-link-arg=-Wl,--undefined=__isoc23_strtol");
        println!("cargo:rustc-link-arg=-Wl,--undefined=__isoc23_strtod");
        println!("cargo:rustc-link-arg=-Wl,--undefined=__isoc23_strtof");
    }
}
