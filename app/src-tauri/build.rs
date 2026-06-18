fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    // libxml2 (>= 2.15) calls BCryptGenRandom from bcrypt.lib on Windows, but the
    // `libxml` crate's build script doesn't request it. Link it explicitly.
    // Harmless on non-Windows targets (this app currently targets Windows only).
    println!("cargo:rustc-link-lib=dylib=bcrypt");

    tauri_build::build()
}
