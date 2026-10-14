fn main() {
    #[cfg(feature = "desktop")]
    tauri_build::build();
    #[cfg(feature = "asio")]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        check_asio_sdk();
    }
}

#[cfg(feature = "asio")]
fn check_asio_sdk() {
    println!("cargo:rerun-if-env-changed=CPAL_ASIO_DIR");
    let sdk = std::path::PathBuf::from(
        std::env::var_os("CPAL_ASIO_DIR")
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| {
                panic!(
                    "CPAL_ASIO_DIR is unset. Run `mise install` (http:asiosdk in mise.toml), or set CPAL_ASIO_DIR to the ASIO SDK root that contains common/asio.h."
                )
            }),
    );
    let asio_h = sdk.join("common").join("asio.h");
    if !asio_h.is_file() {
        panic!(
            "ASIO SDK incomplete at {}: missing {}. Run `mise install` or set CPAL_ASIO_DIR to a complete SDK root.",
            sdk.display(),
            asio_h.display()
        );
    }
}
