//! Spike: prove libxml2 compiles a SEPA schema and validates documents.
//! Also surfaces the `libxml` StructuredError shape (Debug) so the validator
//! module can extract message/line/severity with the correct field names.
use std::path::PathBuf;

use libxml::parser::Parser;
use libxml::schemas::{SchemaParserContext, SchemaValidationContext};

// libxml2 2.15 needs BCryptGenRandom (bcrypt.lib). The `libxml` crate's build
// script omits it, and this integration-test binary doesn't inherit our build.rs
// link directives, so force-link it here. (The real app binary gets it via build.rs.)
#[link(name = "bcrypt")]
extern "C" {}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

#[test]
fn valid_file_passes_schema() {
    let xsd = repo_root().join("xml_schema/pain.008.001.02.xsd");
    let valid = repo_root()
        .join("to_check/valid/20250410_ENRW_ENERGIEVERSORGUNG_ROTTWEIL_GMBH_CO_KG_PAIN00800102.xml");

    assert!(xsd.exists(), "missing xsd fixture: {}", xsd.display());
    assert!(valid.exists(), "missing valid fixture: {}", valid.display());

    let mut sp = SchemaParserContext::from_file(xsd.to_str().unwrap());
    let mut validator = SchemaValidationContext::from_parser(&mut sp)
        .expect("schema must compile (self-contained)");

    let doc = Parser::default()
        .parse_file(valid.to_str().unwrap())
        .expect("valid.xml must parse");

    match validator.validate_document(&doc) {
        Ok(()) => {}
        Err(errors) => {
            for e in &errors {
                eprintln!("UNEXPECTED schema error: {:?}", e);
            }
            panic!("known-good file reported {} schema errors", errors.len());
        }
    }
}

#[test]
fn invalid_file_reports_located_errors() {
    let xsd = repo_root().join("xml_schema/pain.001.001.09.xsd");
    let invalid = repo_root().join("to_check/invalid/20250121_NOFIRMA_PAIN00100109_1.xml");

    assert!(xsd.exists(), "missing xsd fixture: {}", xsd.display());
    assert!(invalid.exists(), "missing invalid fixture: {}", invalid.display());

    let mut sp = SchemaParserContext::from_file(xsd.to_str().unwrap());
    let mut validator = SchemaValidationContext::from_parser(&mut sp)
        .expect("schema must compile");

    let doc = Parser::default()
        .parse_file(invalid.to_str().unwrap())
        .expect("invalid.xml must still parse as XML");

    match validator.validate_document(&doc) {
        Ok(()) => panic!("expected the invalid fixture to fail schema validation"),
        Err(errors) => {
            assert!(!errors.is_empty(), "expected >=1 schema error");
            // Print the full Debug of the first few errors to reveal the
            // StructuredError field/method names for the validator module.
            for e in errors.iter().take(3) {
                eprintln!("STRUCT_ERR_DEBUG: {:?}", e);
            }
        }
    }
}
