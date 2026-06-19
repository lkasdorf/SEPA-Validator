fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    // libxml2 (>= 2.15) needs BCryptGenRandom from bcrypt.lib on Windows.
    println!("cargo:rustc-link-lib=dylib=bcrypt");
    tauri_build::build()
}
