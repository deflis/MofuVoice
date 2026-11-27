fn main() {
    #[cfg(all(windows, feature = "desktop"))]
    {
        println!("cargo:rerun-if-changed=resources/windows.rc");
        println!("cargo:rerun-if-changed=icons/icon.ico");
        let version = env!("CARGO_PKG_VERSION");
        let major = env!("CARGO_PKG_VERSION_MAJOR");
        let minor = env!("CARGO_PKG_VERSION_MINOR");
        let patch = env!("CARGO_PKG_VERSION_PATCH");
        let output =
            std::path::PathBuf::from(std::env::var_os("OUT_DIR").expect("Cargo output directory"));
        std::fs::write(
            output.join("mofu-version.h"),
            format!("#define MOFU_VERSION {major},{minor},{patch},0\n#define MOFU_VERSION_STRING \"{version}\"\n"),
        )
        .expect("Cannot generate MofuVoice version metadata");
        embed_resource::compile("resources/windows.rc", embed_resource::NONE)
            .manifest_optional()
            .expect("Cannot embed MofuVoice icon");
    }
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
