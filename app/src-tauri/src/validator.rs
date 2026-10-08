use std::collections::HashMap;
use std::path::{Path, PathBuf};

use libxml::schemas::{SchemaParserContext, SchemaValidationContext};
use libxml::tree::Document;
use quick_xml::events::{BytesStart, Event};
use quick_xml::name::ResolveResult;
use quick_xml::reader::{NsReader, Reader};

use crate::formatting::format_xml;
use crate::model::{Message, Severity, Status, ValidationResult};
use crate::schema;

/// Returns the namespace URI bound to the first (root) element, or None.
pub fn detect_namespace(path: &Path) -> Option<String> {
    let mut reader = NsReader::from_file(path).ok()?;
    let mut buf = Vec::new();
    loop {
        match reader.read_resolved_event_into(&mut buf) {
            Ok((ResolveResult::Bound(ns), Event::Start(_) | Event::Empty(_))) => {
                return Some(String::from_utf8_lossy(ns.as_ref()).into_owned());
            }
            Ok((_, Event::Start(_) | Event::Empty(_))) => return None, // element but no namespace
            Ok((_, Event::Eof)) => return None,
            Ok(_) => buf.clear(),
            Err(_) => return None,
        }
    }
}

/// True if the file targets a Swiss/Liechtenstein bank, i.e. the Swiss Payment
/// Standards apply: the root's `xsi:schemaLocation` names a `.ch.` schema, or the
/// first debtor account (`DbtrAcct/Id/IBAN`) is a CH/LI IBAN. Creditor accounts
/// don't count (a German debtor paying a Swiss creditor is not a Swiss file).
/// Reading stops at the first debtor account, so large files aren't read in full.
pub fn is_swiss(path: &Path) -> bool {
    let Ok(mut reader) = Reader::from_file(path) else {
        return false;
    };
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut stack: Vec<Vec<u8>> = Vec::new();
    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let name = e.local_name().as_ref().to_vec();
                if stack.is_empty() && root_names_swiss_schema(&e) {
                    return true;
                }
                if name == b"CdtTrfTxInf" {
                    return false; // past the first debtor account
                }
                stack.push(name);
            }
            Ok(Event::Empty(e)) => {
                if stack.is_empty() {
                    return root_names_swiss_schema(&e);
                }
            }
            Ok(Event::End(e)) => {
                if e.local_name().as_ref() == b"DbtrAcct" {
                    return false; // debtor account without an IBAN
                }
                stack.pop();
            }
            Ok(Event::Text(t)) => {
                if stack.ends_with(&[b"DbtrAcct".to_vec(), b"Id".to_vec(), b"IBAN".to_vec()]) {
                    let iban = t.unescape().unwrap_or_default().to_ascii_uppercase();
                    return iban.starts_with("CH") || iban.starts_with("LI");
                }
            }
            Ok(Event::Eof) | Err(_) => return false,
            Ok(_) => {}
        }
    }
}

fn root_names_swiss_schema(root: &BytesStart) -> bool {
    root.attributes().flatten().any(|a| {
        a.key.local_name().as_ref() == b"schemaLocation"
            && String::from_utf8_lossy(&a.value).contains(".ch.")
    })
}

/// Holds a per-run cache of compiled schemas. Not Send (wraps libxml2 pointers):
/// construct and use it on a single worker thread.
pub struct Validator {
    schema_dir: PathBuf,
    cache: HashMap<&'static str, SchemaValidationContext>,
}

impl Validator {
    /// `schema_dir` holds the imported XSD files looked up by filename.
    pub fn new(schema_dir: PathBuf) -> Self {
        Self {
            schema_dir,
            cache: HashMap::new(),
        }
    }

    pub fn validate_file(&mut self, path: &Path) -> ValidationResult {
        let file = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        let path_str = path.display().to_string();

        let mk = |ns: String,
                  schema_name: String,
                  msgs: Vec<Message>,
                  status: Status,
                  e: u32,
                  w: u32| {
            ValidationResult {
                file: file.clone(),
                path: path_str.clone(),
                namespace: ns,
                schema: schema_name,
                status,
                errors: e,
                warnings: w,
                messages: msgs,
            }
        };

        if !path.exists() {
            return mk(
                String::new(),
                String::new(),
                vec![Message {
                    severity: Severity::Error,
                    text: "File not found.".into(),
                    line: None,
                    column: None,
                }],
                Status::Error,
                1,
                0,
            );
        }

        let ns = match detect_namespace(path) {
            Some(ns) => ns,
            None => {
                return mk(
                    String::new(),
                    String::new(),
                    vec![Message {
                        severity: Severity::Error,
                        text: "No XML namespace detected. File may not be valid XML.".into(),
                        line: None,
                        column: None,
                    }],
                    Status::Error,
                    1,
                    0,
                )
            }
        };

        let swiss = schema::has_swiss_variant(&ns) && is_swiss(path);
        let schema_name = match schema::resolve(&ns, swiss) {
            Some(name) => name,
            None => {
                return mk(
                    ns.clone(),
                    String::new(),
                    vec![Message {
                        severity: Severity::Warning,
                        text: format!("No matching schema for namespace: {ns}"),
                        line: None,
                        column: None,
                    }],
                    Status::NoSchema,
                    0,
                    1,
                )
            }
        };

        if !self.schema_dir.join(schema_name).exists() {
            return mk(
                ns.clone(),
                schema_name.to_string(),
                vec![Message {
                    severity: Severity::Warning,
                    text: format!("Schema '{schema_name}' not imported. Open Schemas… to import it."),
                    line: None,
                    column: None,
                }],
                Status::NoSchema,
                0,
                1,
            );
        }

        if !self.cache.contains_key(schema_name) {
            match self.compile(schema_name) {
                Ok(ctx) => {
                    self.cache.insert(schema_name, ctx);
                }
                Err(text) => {
                    return mk(
                        ns.clone(),
                        schema_name.to_string(),
                        vec![Message {
                            severity: Severity::Error,
                            text,
                            line: None,
                            column: None,
                        }],
                        Status::Error,
                        1,
                        0,
                    )
                }
            }
        }
        let validator = self.cache.get_mut(schema_name).unwrap();

        // Format first, then validate the formatted text so that libxml's
        // reported line/col numbers match what the viewer shows (`read_formatted`).
        let formatted = match format_xml(path) {
            Ok(s) => s,
            Err(e) => {
                return mk(
                    ns.clone(),
                    schema_name.to_string(),
                    vec![Message {
                        severity: Severity::Error,
                        text: e.message,
                        line: e.line,
                        column: e.column,
                    }],
                    Status::Error,
                    1,
                    0,
                )
            }
        };
        let doc = match parse_strict(&formatted) {
            Ok(d) => d,
            Err(m) => {
                return mk(
                    ns.clone(),
                    schema_name.to_string(),
                    vec![m],
                    Status::Error,
                    1,
                    0,
                )
            }
        };

        // Call libxml directly: the crate's `validate_document` panics on rc == -1
        // (e.g. entity nodes) and only drains the error log on failure.
        let rc =
            unsafe { libxml::bindings::xmlSchemaValidateDoc(validator.as_ptr(), doc.doc_ptr()) };
        let mut messages: Vec<Message> = validator.drain_errors().iter().map(to_message).collect();
        if rc != 0 && !messages.iter().any(|m| m.severity == Severity::Error) {
            messages.push(Message {
                severity: Severity::Error,
                text: format!("Schema validation failed (libxml2 code {rc})."),
                line: None,
                column: None,
            });
        }

        ValidationResult::from_messages(file, path_str, ns, schema_name.to_string(), messages)
    }

    fn compile(&self, filename: &str) -> Result<SchemaValidationContext, String> {
        let path = self.schema_dir.join(filename);
        let path_str = path.to_str().ok_or("schema path is not valid UTF-8")?;
        let mut parser = SchemaParserContext::from_file(path_str);
        SchemaValidationContext::from_parser(&mut parser)
            .map_err(|errs| format!("Failed to load schema: {} error(s)", errs.len()))
    }
}

/// Strict parse of the formatted text: no error recovery (a repaired tree would
/// hide well-formedness errors), no network, and line numbers beyond 65535.
fn parse_strict(text: &str) -> Result<Document, Message> {
    use libxml::bindings as b;
    let fail = |text: String| Message {
        severity: Severity::Error,
        text,
        line: None,
        column: None,
    };
    let len = i32::try_from(text.len())
        .map_err(|_| fail("File too large to validate (over 2 GB).".into()))?;
    let options = b::xmlParserOption_XML_PARSE_NONET
        | b::xmlParserOption_XML_PARSE_BIG_LINES
        | b::xmlParserOption_XML_PARSE_NOERROR
        | b::xmlParserOption_XML_PARSE_NOWARNING;
    unsafe {
        let ctxt = b::xmlNewParserCtxt();
        if ctxt.is_null() {
            return Err(fail("Could not create the XML parser.".into()));
        }
        let doc = b::xmlCtxtReadMemory(
            ctxt,
            text.as_ptr().cast(),
            len,
            std::ptr::null(),
            c"UTF-8".as_ptr(),
            options,
        );
        let result = if doc.is_null() {
            let err = b::xmlCtxtGetLastError(ctxt.cast());
            Err(if err.is_null() {
                fail("XML not well-formed.".into())
            } else {
                let mut m = to_message(&libxml::error::StructuredError::from_raw(err));
                m.severity = Severity::Error;
                m.text = format!("XML not well-formed: {}", m.text);
                m
            })
        } else {
            Ok(Document::new_ptr(doc))
        };
        b::xmlFreeParserCtxt(ctxt);
        result
    }
}

/// Map a libxml StructuredError to our Message.
fn to_message(e: &libxml::error::StructuredError) -> Message {
    use libxml::error::XmlErrorLevel;
    let severity = match e.level {
        XmlErrorLevel::Warning => Severity::Warning,
        _ => Severity::Error,
    };
    let text = e
        .message
        .clone()
        .unwrap_or_else(|| "validation error".into())
        .trim()
        .to_string();
    let line = e.line.filter(|l| *l > 0).map(|l| l as u32);
    let column = e.col.filter(|c| *c > 0).map(|c| c as u32);
    Message {
        severity,
        text,
        line,
        column,
    }
}

#[cfg(test)]
mod strict_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn temp_xml(contents: &str) -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("sepa_ns_test_{}.xml", contents.len()));
        let mut f = std::fs::File::create(&p).unwrap();
        f.write_all(contents.as_bytes()).unwrap();
        p
    }

    #[test]
    fn detects_default_namespace_on_root() {
        let p = temp_xml(
            r#"<?xml version="1.0"?><Document xmlns="urn:iso:std:iso:20022:tech:xsd:pain.008.001.02"><X/></Document>"#,
        );
        assert_eq!(
            detect_namespace(&p).as_deref(),
            Some("urn:iso:std:iso:20022:tech:xsd:pain.008.001.02")
        );
    }

    #[test]
    fn returns_none_for_no_namespace() {
        let p = temp_xml(r#"<?xml version="1.0"?><root><child/></root>"#);
        assert_eq!(detect_namespace(&p), None);
    }

    #[test]
    fn returns_none_for_garbage() {
        let p = temp_xml("not xml at all <<<");
        assert_eq!(detect_namespace(&p), None);
    }

    /// Temp file with a caller-chosen name (avoids clashes between same-length fixtures).
    fn temp_named(tag: &str, contents: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("sepa_swiss_test_{tag}.xml"));
        std::fs::write(&p, contents).unwrap();
        p
    }

    /// Minimal pain.001.001.09 skeleton: debtor IBAN, creditor IBAN, root attributes.
    fn pain001(root_attrs: &str, debtor_iban: &str, creditor_iban: &str) -> String {
        format!(
            r#"<?xml version="1.0"?><Document xmlns="urn:iso:std:iso:20022:tech:xsd:pain.001.001.09" {root_attrs}><CstmrCdtTrfInitn><PmtInf><PmtInfId>P1</PmtInfId><Dbtr><Nm>A</Nm></Dbtr><DbtrAcct><Id><IBAN>{debtor_iban}</IBAN></Id></DbtrAcct><CdtTrfTxInf><CdtrAcct><Id><IBAN>{creditor_iban}</IBAN></Id></CdtrAcct></CdtTrfTxInf></PmtInf></CstmrCdtTrfInitn></Document>"#
        )
    }

    #[test]
    fn swiss_debtor_iban_is_swiss() {
        let p = temp_named("ch_debtor", &pain001("", "CH9300762011623852957", "DE89370400440532013000"));
        assert!(is_swiss(&p));
    }

    #[test]
    fn liechtenstein_debtor_iban_is_swiss() {
        let p = temp_named("li_debtor", &pain001("", "LI21088100002324013AA", "DE89370400440532013000"));
        assert!(is_swiss(&p));
    }

    #[test]
    fn german_debtor_paying_swiss_creditor_is_not_swiss() {
        let p = temp_named("de_debtor", &pain001("", "DE89370400440532013000", "CH9300762011623852957"));
        assert!(!is_swiss(&p));
    }

    #[test]
    fn swiss_schema_location_is_swiss() {
        let attrs = r#"xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xsi:schemaLocation="urn:iso:std:iso:20022:tech:xsd:pain.001.001.09 pain.001.001.09.ch.03.xsd""#;
        let p = temp_named("ch_location", &pain001(attrs, "DE89370400440532013000", "DE89370400440532013000"));
        assert!(is_swiss(&p));
    }

    #[test]
    fn swiss_file_resolves_to_swiss_schema() {
        let empty = std::env::temp_dir().join("sepa_swiss_test_empty_schemas");
        std::fs::create_dir_all(&empty).unwrap();
        let mut v = super::Validator::new(empty);

        let ch = temp_named("resolve_ch", &pain001("", "CH9300762011623852957", "CH9300762011623852957"));
        let r = v.validate_file(&ch);
        assert_eq!(r.schema, "pain.001.001.09.ch.03.xsd");
        assert_eq!(r.status, Status::NoSchema);

        let de = temp_named("resolve_de", &pain001("", "DE89370400440532013000", "CH9300762011623852957"));
        assert_eq!(v.validate_file(&de).schema, "pain.001.001.09.xsd");
    }

    #[test]
    fn swiss_credit_transfer_validates_against_swiss_schema() {
        let dir = repo_root().join("xml_schema").join("ch");
        if !dir.join("pain.001.001.09.ch.03.xsd").exists() {
            eprintln!("SKIP: xml_schema/ch/pain.001.001.09.ch.03.xsd absent");
            return;
        }
        let p = temp_named(
            "ch_valid",
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:pain.001.001.09"><CstmrCdtTrfInitn>
<GrpHdr><MsgId>MSG-CH-1</MsgId><CreDtTm>2026-09-19T10:00:00</CreDtTm><NbOfTxs>1</NbOfTxs><CtrlSum>100.00</CtrlSum><InitgPty><Nm>Muster AG</Nm></InitgPty></GrpHdr>
<PmtInf><PmtInfId>PMT-1</PmtInfId><PmtMtd>TRF</PmtMtd><ReqdExctnDt><Dt>2026-09-21</Dt></ReqdExctnDt>
<Dbtr><Nm>Muster AG</Nm></Dbtr><DbtrAcct><Id><IBAN>CH9300762011623852957</IBAN></Id></DbtrAcct>
<DbtrAgt><FinInstnId><BICFI>UBSWCHZH80A</BICFI></FinInstnId></DbtrAgt>
<CdtTrfTxInf><PmtId><InstrId>INSTR-1</InstrId><EndToEndId>E2E-1</EndToEndId></PmtId>
<Amt><InstdAmt Ccy="CHF">100.00</InstdAmt></Amt><Cdtr><Nm>Beispiel GmbH</Nm></Cdtr>
<CdtrAcct><Id><IBAN>CH5604835012345678009</IBAN></Id></CdtrAcct></CdtTrfTxInf>
</PmtInf></CstmrCdtTrfInitn></Document>"#,
        );
        let r = super::Validator::new(dir).validate_file(&p);
        assert_eq!(r.schema, "pain.001.001.09.ch.03.xsd");
        assert_eq!(r.status, Status::Ok, "messages: {:?}", r.messages);
    }

    #[test]
    fn official_swiss_direct_debit_examples_are_ok() {
        let dir = repo_root().join("xml_schema").join("ch");
        let examples = dir.join("examples");
        if !examples.exists() {
            eprintln!("SKIP: xml_schema/ch/examples absent");
            return;
        }
        let mut v = super::Validator::new(dir);
        for (file, schema) in [
            ("pain_008_Swiss-DD_Beispiel_1.xml", "pain.008.001.02.ch.03.xsd"),
            ("pain_008_Beispiel_1.xml", "pain.008.001.02.chsdd.02.xsd"),
        ] {
            let r = v.validate_file(&examples.join(file));
            assert_eq!(r.schema, schema);
            assert_eq!(r.status, Status::Ok, "{file}: {:?}", r.messages);
        }
    }

    use crate::model::Status;

    fn repo_root() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
    }

    /// Local XSD directory (never committed); tests that compile schemas skip if absent.
    fn test_schema_dir() -> std::path::PathBuf {
        repo_root().join("xml_schema")
    }

    #[test]
    fn unknown_namespace_yields_no_schema() {
        let p = temp_xml(r#"<?xml version="1.0"?><Doc xmlns="urn:made:up"><X/></Doc>"#);
        let mut v = super::Validator::new(test_schema_dir());
        let r = v.validate_file(&p);
        assert_eq!(r.status, Status::NoSchema);
        assert_eq!(r.namespace, "urn:made:up");
    }

    #[test]
    fn valid_fixture_is_ok() {
        let f = repo_root().join(
            "to_check/valid/20250410_ENRW_ENERGIEVERSORGUNG_ROTTWEIL_GMBH_CO_KG_PAIN00800102.xml",
        );
        if !f.exists() || !test_schema_dir().exists() {
            eprintln!("SKIP: fixture or schema dir absent");
            return;
        }
        let mut v = super::Validator::new(test_schema_dir());
        let r = v.validate_file(&f);
        assert_eq!(r.status, Status::Ok, "messages: {:?}", r.messages);
    }

    #[test]
    fn invalid_fixture_reports_errors() {
        let f = repo_root().join("to_check/invalid/20250121_NOFIRMA_PAIN00100109_1.xml");
        if !f.exists() || !test_schema_dir().exists() {
            eprintln!("SKIP: fixture or schema dir absent");
            return;
        }
        let mut v = super::Validator::new(test_schema_dir());
        let r = v.validate_file(&f);
        assert_eq!(r.status, Status::Invalid);
        assert!(r.errors >= 1);
        assert!(
            r.messages.iter().any(|m| m.line.is_some()),
            "expect at least one located error"
        );
    }

    #[test]
    fn invalid_fixture_lines_point_into_formatted_text() {
        let f = repo_root().join("to_check/invalid/20250121_NOFIRMA_PAIN00100109_1.xml");
        if !f.exists() || !test_schema_dir().exists() {
            eprintln!("SKIP: fixture or schema dir absent");
            return;
        }
        let formatted = crate::formatting::format_xml(&f).unwrap();
        let line_count = formatted.lines().count() as u32;
        let mut v = super::Validator::new(test_schema_dir());
        let r = v.validate_file(&f);
        assert_eq!(r.status, Status::Invalid);
        for m in r.messages.iter() {
            if let Some(line) = m.line {
                assert!(
                    line >= 1 && line <= line_count,
                    "error line {line} outside formatted text (1..={line_count})"
                );
            }
        }
        assert!(r.messages.iter().any(|m| m.line.is_some()));
    }
}
