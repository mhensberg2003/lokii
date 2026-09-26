use std::env;
use std::path::{Path, PathBuf};

fn main() {
    link_libmpv();
    tauri_build::build()
}

/// Links the libmpv in `libmpv/<os>`, which `scripts/fetch-libmpv.sh` downloads. Without
/// it, dev builds on macOS link Homebrew's libmpv.
fn link_libmpv() {
    println!("cargo:rerun-if-changed=libmpv");
    let os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let vendored = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("libmpv").join(&os);
    match os.as_str() {
        "macos" if vendored.join("libmpv.dylib").exists() => link_macos(&vendored),
        "windows" if vendored.join("libmpv-2.dll").exists() => link_windows(&vendored),
        "macos" => {
            for dir in ["/opt/homebrew/lib", "/usr/local/lib"] {
                if Path::new(dir).exists() {
                    println!("cargo:rustc-link-search=native={dir}");
                }
            }
        }
        _ => {}
    }
}

/// The app bundle keeps the dylibs in Contents/Frameworks (see tauri.macos.conf.json).
/// Debug builds load them from `libmpv/macos`.
fn link_macos(vendored: &Path) {
    println!("cargo:rustc-link-search=native={}", vendored.display());
    println!("cargo:rustc-link-arg=-Wl,-rpath,@executable_path/../Frameworks");
    if env::var("PROFILE").as_deref() == Ok("debug") {
        println!("cargo:rustc-link-arg=-Wl,-rpath,{}", vendored.display());
    }
}

/// MSVC links `mpv.lib`; the MinGW import library `libmpv.dll.a` works as one. The DLL
/// goes next to the debug executable, and the installer puts it next to Lokii.exe
/// (see tauri.windows.conf.json).
fn link_windows(vendored: &Path) {
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    std::fs::copy(vendored.join("libmpv.dll.a"), out.join("mpv.lib")).expect("copy the libmpv import library");
    println!("cargo:rustc-link-search=native={}", out.display());
    // OUT_DIR is target/<profile>/build/<crate>/out.
    if let Some(profile_dir) = out.ancestors().nth(3) {
        let _ = std::fs::copy(vendored.join("libmpv-2.dll"), profile_dir.join("libmpv-2.dll"));
    }
}
