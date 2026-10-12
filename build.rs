fn main() {
    println!("cargo:rerun-if-env-changed=CPAL_ASIO_DIR");
    println!("cargo:rerun-if-changed=native/asio_extra.cpp");
    println!("cargo:rerun-if-changed=native/gui.cpp");
    println!("cargo:rustc-link-lib=user32");
    println!("cargo:rustc-link-lib=gdi32");
    println!("cargo:rustc-link-lib=ole32");
    println!("cargo:rustc-link-lib=comctl32");
    println!("cargo:rustc-link-lib=dwmapi");
    let sdk = std::path::PathBuf::from(std::env::var_os("CPAL_ASIO_DIR").unwrap_or_else(|| {
        panic!(
            "CPAL_ASIO_DIR is unset. Place the ASIO SDK at .tools/asiosdk (see .cargo/config.toml and mise.toml), or set CPAL_ASIO_DIR to that directory."
        )
    }));
    let asio_h = sdk.join("common").join("asio.h");
    if !asio_h.is_file() {
        panic!(
            "ASIO SDK incomplete at {}: missing {}. Install the SDK there or set CPAL_ASIO_DIR.",
            sdk.display(),
            asio_h.display()
        );
    }
    cc::Build::new()
        .cpp(true)
        .std("c++17")
        .include(sdk.join("common"))
        .file("native/asio_extra.cpp")
        .file("native/gui.cpp")
        .flag_if_supported("/utf-8")
        .compile("voice_asio_extra");
}
