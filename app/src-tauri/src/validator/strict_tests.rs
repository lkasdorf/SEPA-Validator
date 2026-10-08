//! Regression tests for verdict correctness: malformed or subtly invalid files
//! must never come back OK, and hostile input must not crash the worker.
//! Uses the self-written mini schema in `tests/fixtures/schemas` (no ISO XSDs needed).

use super::Validator;
use crate::model::{Status, ValidationResult};

const NS: &str = "urn:iso:std:iso:20022:tech:xsd:pain.008.001.02";

fn fixture_schemas() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("schemas")
}

fn tx(nm: &str, iban: &str, bic: &str) -> String {
    format!("<Tx><Nm>{nm}</Nm><IBAN>{iban}</IBAN><BIC>{bic}</BIC></Tx>")
}

fn good_tx() -> String {
    tx("Muster GmbH", "DE89370400440532013000", "COBADEFFXXX")
}

/// Single-line document, like most SEPA files produced by banking software.
fn doc(body: &str) -> String {
    format!(r#"<?xml version="1.0" encoding="UTF-8"?><Document xmlns="{NS}">{body}</Document>"#)
}

fn validate(tag: &str, contents: &str) -> ValidationResult {
    let p = std::env::temp_dir().join(format!("sepa_strict_test_{tag}.xml"));
    std::fs::write(&p, contents).unwrap();
    Validator::new(fixture_schemas()).validate_file(&p)
}

#[test]
fn valid_minimal_document_is_ok() {
    let r = validate("valid", &doc(&good_tx()));
    assert_eq!(r.status, Status::Ok, "messages: {:?}", r.messages);
}

#[test]
fn whitespace_only_name_is_ok() {
    // Not over-strict: a single space satisfies minLength=1 in the original file.
    let r = validate(
        "ws_name",
        &doc(&tx(" ", "DE89370400440532013000", "COBADEFFXXX")),
    );
    assert_eq!(r.status, Status::Ok, "messages: {:?}", r.messages);
}

#[test]
fn truncated_file_is_not_ok() {
    let full = doc(&format!("{}{}", good_tx(), good_tx()));
    let cut = full.trim_end_matches("</Document>");
    let r = validate("truncated", cut);
    assert_eq!(r.status, Status::Error, "messages: {:?}", r.messages);
}

#[test]
fn bare_ampersand_is_not_ok() {
    let r = validate(
        "amp",
        &doc(&tx("Muster & Co", "DE89370400440532013000", "COBADEFFXXX")),
    );
    assert_eq!(r.status, Status::Error, "messages: {:?}", r.messages);
}

#[test]
fn well_formedness_error_is_located() {
    let r = validate(
        "amp_loc",
        &doc(&tx("Muster & Co", "DE89370400440532013000", "COBADEFFXXX")),
    );
    assert!(
        r.messages.iter().any(|m| m.line.is_some()),
        "messages: {:?}",
        r.messages
    );
}

#[test]
fn trailing_garbage_is_not_ok() {
    let r = validate("garbage", &format!("{}garbage", doc(&good_tx())));
    assert_eq!(r.status, Status::Error, "messages: {:?}", r.messages);
}

#[test]
fn second_root_element_is_not_ok() {
    let second = format!(r#"<Document xmlns="{NS}">{}</Document>"#, good_tx());
    let r = validate("two_roots", &format!("{}{second}", doc(&good_tx())));
    assert_eq!(r.status, Status::Error, "messages: {:?}", r.messages);
}

#[test]
fn control_character_is_not_ok() {
    let r = validate(
        "ctrl",
        &doc(&tx(
            "Muster\u{1}GmbH",
            "DE89370400440532013000",
            "COBADEFFXXX",
        )),
    );
    assert_eq!(r.status, Status::Error, "messages: {:?}", r.messages);
}

#[test]
fn iban_with_leading_space_is_invalid() {
    let r = validate(
        "iban_space",
        &doc(&tx("Muster GmbH", " DE89370400440532013000", "COBADEFFXXX")),
    );
    assert_eq!(r.status, Status::Invalid, "messages: {:?}", r.messages);
}

#[test]
fn bic_with_trailing_space_is_invalid() {
    let r = validate(
        "bic_space",
        &doc(&tx("Muster GmbH", "DE89370400440532013000", "COBADEFFXXX ")),
    );
    assert_eq!(r.status, Status::Invalid, "messages: {:?}", r.messages);
}

#[test]
fn empty_name_is_invalid() {
    let r = validate(
        "empty_nm",
        &doc(&tx("", "DE89370400440532013000", "COBADEFFXXX")),
    );
    assert_eq!(r.status, Status::Invalid, "messages: {:?}", r.messages);
}

#[test]
fn doctype_with_entity_is_rejected_without_panic() {
    let body = tx("&x;", "DE89370400440532013000", "COBADEFFXXX");
    let contents = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><!DOCTYPE Document [<!ENTITY x "Muster">]><Document xmlns="{NS}">{body}</Document>"#
    );
    let r = validate("doctype", &contents);
    assert_eq!(r.status, Status::Error, "messages: {:?}", r.messages);
}

#[test]
fn undefined_entity_is_rejected_without_panic() {
    let r = validate(
        "undef_entity",
        &doc(&tx("&foo;", "DE89370400440532013000", "COBADEFFXXX")),
    );
    assert_eq!(r.status, Status::Error, "messages: {:?}", r.messages);
}

#[test]
fn error_line_beyond_65535_is_reported_exactly() {
    // ~5 formatted lines per Tx -> well past libxml's 16-bit line limit.
    let mut body = good_tx().repeat(16_000);
    body.push_str(&tx("Muster GmbH", "BAD-IBAN", "COBADEFFXXX"));
    let contents = doc(&body);
    let p = std::env::temp_dir().join("sepa_strict_test_big_lines.xml");
    std::fs::write(&p, &contents).unwrap();

    let formatted = crate::formatting::format_xml(&p).unwrap();
    let expected = formatted
        .lines()
        .position(|l| l.contains("BAD-IBAN"))
        .map(|i| i as u32 + 1)
        .unwrap();
    assert!(expected > 65_535, "fixture too small: line {expected}");

    let r = Validator::new(fixture_schemas()).validate_file(&p);
    assert_eq!(r.status, Status::Invalid, "messages: {:?}", r.messages);
    assert_eq!(
        r.messages[0].line,
        Some(expected),
        "messages: {:?}",
        r.messages
    );
}

#[test]
fn schema_errors_are_readable_and_carry_a_hint() {
    let r = validate(
        "readable",
        &doc(&tx("Muster GmbH", " DE89370400440532013000", "COBADEFFXXX")),
    );
    let m = &r.messages[0];
    assert!(!m.text.contains("{urn:"), "namespace left in: {}", m.text);
    assert!(m.text.starts_with("Element 'IBAN'"), "got: {}", m.text);
    assert_eq!(
        m.hint.as_deref(),
        Some("The value has leading or trailing spaces; remove them.")
    );
}
