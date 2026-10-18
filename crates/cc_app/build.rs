//! Embeds the application icon in the Windows executable.

fn main() {
    println!("cargo:rerun-if-changed=../../assets/icons/app-icon.ico");
    #[cfg(windows)]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("../../assets/icons/app-icon.ico");
        if let Err(err) = res.compile() {
            // A missing resource compiler must not break the build; the
            // window icon is still set at runtime.
            println!("cargo:warning=could not embed the application icon: {err}");
        }
    }
}
