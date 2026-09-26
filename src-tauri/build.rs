fn main() {
    // Dev builds link against Homebrew's libmpv. Release builds bundle libmpv (milestone M6).
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        for dir in ["/opt/homebrew/lib", "/usr/local/lib"] {
            if std::path::Path::new(dir).exists() {
                println!("cargo:rustc-link-search=native={dir}");
            }
        }
    }
    tauri_build::build()
}
