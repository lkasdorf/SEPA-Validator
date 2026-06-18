use std::path::Path;

// Physical XSD filenames that must be present in <repo>/xml_schema for embedding.
const SCHEMA_FILES: &[&str] = &[
    "pain.001.001.03.xsd",
    "pain.001.001.09.xsd",
    "pain.002.001.10.xsd",
    "pain.007.001.09_GBIC_5.xsd",
    "pain.008.001.02.xsd",
    "pain.008.001.08.xsd",
    "camt.054.001.08.xsd",
    "container.nnn.001.GBIC4.xsd",
];

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    // libxml2 (>= 2.15) needs BCryptGenRandom from bcrypt.lib on Windows.
    println!("cargo:rustc-link-lib=dylib=bcrypt");

    let schema_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("xml_schema");
    println!("cargo:rerun-if-changed={}", schema_dir.display());
    for f in SCHEMA_FILES {
        let p = schema_dir.join(f);
        if !p.exists() {
            panic!(
                "Required schema file missing: {}\nPlace the ISO 20022 / GBIC XSDs in xml_schema/.",
                p.display()
            );
        }
        println!("cargo:rerun-if-changed={}", p.display());
    }

    tauri_build::build()
}
