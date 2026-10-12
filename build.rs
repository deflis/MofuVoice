fn main() {
    println!("cargo:rerun-if-env-changed=CPAL_ASIO_DIR");
    println!("cargo:rerun-if-changed=native/asio_extra.cpp");
    println!("cargo:rerun-if-changed=native/gui.cpp");
    println!("cargo:rustc-link-lib=user32");
    println!("cargo:rustc-link-lib=gdi32");
    println!("cargo:rustc-link-lib=ole32");
    let sdk = std::path::PathBuf::from(
        std::env::var_os("CPAL_ASIO_DIR").expect("Set CPAL_ASIO_DIR to the ASIO SDK directory"),
    );
    cc::Build::new()
        .cpp(true)
        .std("c++17")
        .include(sdk.join("common"))
        .file("native/asio_extra.cpp")
        .file("native/gui.cpp")
        .flag_if_supported("/utf-8")
        .compile("voice_asio_extra");
}
