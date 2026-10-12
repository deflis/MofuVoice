fn main() {
    #[cfg(feature = "desktop")]
    tauri_build::build();
    println!("cargo:rerun-if-env-changed=CPAL_ASIO_DIR");
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
}
