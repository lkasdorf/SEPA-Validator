use std::collections::HashMap;
use std::io::BufRead;
use std::path::{Path, PathBuf};

use libxml::schemas::{SchemaParserContext, SchemaValidationContext};
use libxml::tree::Document;
use quick_xml::events::{BytesStart, Event};
use quick_xml::name::ResolveResult;
use quick_xml::reader::{NsReader, Reader};

use crate::formatting::format_bytes;
use crate::model::{Message, Severity, Status, ValidationResult};
use crate::schema;

/// Returns the namespace URI bound to the first (root) element, or None.
pub fn detect_namespace(path: &Path) -> Option<String> {
    namespace_of(NsReader::from_file(path).ok()?)
}

fn namespace_of<R: BufRead>(mut reader: NsReader<R>) -> Option<String> {
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
pub fn is_swiss(xml: &[u8]) -> bool {
    let mut reader = Reader::from_reader(xml);
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

/// Which schema variant a file needs when its namespace is shared (see
/// `schema::Variant`). Swiss wins: SPS files may carry CHF, SHAR etc.
pub fn detect_variant(xml: &[u8]) -> schema::Variant {
    if is_swiss(xml) {
        schema::Variant::Swiss
    } else {
        dk_credit_transfer_kind(xml)
    }
}

/// Tells the three DK pain.001 subsets apart:
/// - SEPA (standard): EUR, `ChrgBr` SLEV, service level SEPA.
/// - Urgent euro (CCU): EUR, `ChrgBr` SHAR, service level URGP.
/// - Foreign (AXZ): any currency, a per-transaction `ChrgBr` DEBT/CRED/SHAR and
///   service level NURG/URGP/SDVA, optionally an equivalent amount or a cheque.
///
/// A non-EUR amount, `EqvtAmt`, a cheque, `ChrgBr` DEBT/CRED, a service level
/// other than SEPA/URGP or an `xsi:schemaLocation` naming AXZ make it foreign
/// (reading stops there). Otherwise URGP, SHAR or a CCU schemaLocation make it
/// urgent, and anything else is SEPA. The few files valid against both CCU and
/// AXZ (EUR, URGP, SHAR) are guessed as CCU; the validator then falls back to the
/// other DK schemas if the guess fails (`dk_candidates`).
pub fn dk_credit_transfer_kind(xml: &[u8]) -> schema::Variant {
    use schema::Variant;
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut stack: Vec<Vec<u8>> = Vec::new();
    let mut urgent = false;
    loop {
        buf.clear();
        let foreign = match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let signal = element_signal(&e, stack.is_empty());
                stack.push(e.local_name().as_ref().to_vec());
                signal
            }
            Ok(Event::Empty(e)) => element_signal(&e, stack.is_empty()),
            Ok(Event::End(_)) => {
                stack.pop();
                Signal::None
            }
            Ok(Event::Text(t)) => {
                let value = t.unescape().unwrap_or_default();
                text_signal(&stack, value.trim())
            }
            Ok(Event::Eof) | Err(_) => break,
            Ok(_) => Signal::None,
        };
        match foreign {
            Signal::Foreign => return Variant::Foreign,
            Signal::Urgent => urgent = true,
            Signal::None => {}
        }
    }
    if urgent {
        Variant::Urgent
    } else {
        Variant::Standard
    }
}

enum Signal {
    None,
    Urgent,
    Foreign,
}

fn element_signal(e: &BytesStart, is_root: bool) -> Signal {
    let attr = |name: &[u8]| {
        e.attributes()
            .flatten()
            .find(|a| a.key.local_name().as_ref() == name)
            .map(|a| String::from_utf8_lossy(&a.value).to_ascii_uppercase())
    };
    match e.local_name().as_ref() {
        _ if is_root => match attr(b"schemaLocation") {
            Some(l) if l.contains("AXZ") => Signal::Foreign,
            Some(l) if l.contains("CCU") => Signal::Urgent,
            _ => Signal::None,
        },
        b"EqvtAmt" => Signal::Foreign,
        b"InstdAmt" if attr(b"Ccy").is_some_and(|c| c.trim() != "EUR") => Signal::Foreign,
        _ => Signal::None,
    }
}

fn text_signal(stack: &[Vec<u8>], value: &str) -> Signal {
    let (parent, element) = match stack {
        [.., p, e] => (p.as_slice(), e.as_slice()),
        _ => return Signal::None,
    };
    match (parent, element, value) {
        (_, b"ChrgBr", "DEBT" | "CRED") => Signal::Foreign,
        (_, b"ChrgBr", "SHAR") => Signal::Urgent,
        (_, b"PmtMtd", "CHK" | "TRA") => Signal::Foreign,
        (b"SvcLvl", b"Cd", "URGP") => Signal::Urgent,
        (b"SvcLvl", b"Cd", "SEPA") => Signal::None,
        (b"SvcLvl", b"Cd", _) => Signal::Foreign,
        _ => Signal::None,
    }
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
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) => {
                let text = if path.exists() {
                    format!("Cannot read the file: {e}")
                } else {
                    "File not found.".into()
                };
                return ValidationResult {
                    file: file_name_of(path),
                    path: path.display().to_string(),
                    namespace: String::new(),
                    schema: String::new(),
                    status: Status::Error,
                    errors: 1,
                    warnings: 0,
                    messages: vec![Message {
                        severity: Severity::Error,
                        text,
                        line: None,
                        column: None,
                        hint: None,
                    }],
                };
            }
        };
        self.validate_bytes(path, &bytes)
    }

    /// Validate XML held in memory as if it were the content of `path` (which
    /// only names the result; it is not read). Used for text edited in the viewer.
    pub fn validate_bytes(&mut self, path: &Path, bytes: &[u8]) -> ValidationResult {
        let file = file_name_of(path);
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

        let ns = match namespace_of(NsReader::from_reader(bytes)) {
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
                        hint: None,
                    }],
                    Status::Error,
                    1,
                    0,
                )
            }
        };

        let variant = if schema::has_variants(&ns) {
            detect_variant(bytes)
        } else {
            schema::Variant::Standard
        };
        let schema_name = match schema::resolve(&ns, variant) {
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
                        hint: None,
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
                    text: format!(
                        "Schema '{schema_name}' not imported. Open Schemas… to import it."
                    ),
                    line: None,
                    column: None,
                    hint: None,
                }],
                Status::NoSchema,
                0,
                1,
            );
        }

        // Format first, then validate the formatted text so that libxml's
        // reported line/col numbers match what the viewer shows (`read_formatted`).
        let formatted = match format_bytes(bytes) {
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
                        hint: None,
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

        let mut schema_name = schema_name;
        let mut messages = match self.schema_messages(schema_name, &doc) {
            Ok(m) => m,
            Err(text) => {
                return mk(
                    ns.clone(),
                    schema_name.to_string(),
                    vec![Message {
                        severity: Severity::Error,
                        text,
                        line: None,
                        column: None,
                        hint: None,
                    }],
                    Status::Error,
                    1,
                    0,
                )
            }
        };
        // The DK subset was guessed from the content; if the guess fails, a file
        // that is valid against another DK subset of the same namespace is that one.
        if has_errors(&messages) && variant != schema::Variant::Swiss {
            for alt in schema::dk_candidates(&ns) {
                if alt == schema_name || !self.schema_dir.join(alt).exists() {
                    continue;
                }
                if let Ok(m) = self.schema_messages(alt, &doc) {
                    if !has_errors(&m) {
                        schema_name = alt;
                        messages = m;
                        break;
                    }
                }
            }
        }
        // Checks the schema can't express (control sums, IBAN check digits, ...).
        messages.extend(crate::plausibility::check(
            &formatted,
            &crate::plausibility::today(),
        ));

        ValidationResult::from_messages(file, path_str, ns, schema_name.to_string(), messages)
    }

    /// XSD validation messages for `doc` against an imported schema (compiled once per run).
    fn schema_messages(
        &mut self,
        schema_name: &'static str,
        doc: &Document,
    ) -> Result<Vec<Message>, String> {
        if !self.cache.contains_key(schema_name) {
            let ctx = self.compile(schema_name)?;
            self.cache.insert(schema_name, ctx);
        }
        let validator = self.cache.get_mut(schema_name).unwrap();
        // Call libxml directly: the crate's `validate_document` panics on rc == -1
        // (e.g. entity nodes) and only drains the error log on failure.
        let rc =
            unsafe { libxml::bindings::xmlSchemaValidateDoc(validator.as_ptr(), doc.doc_ptr()) };
        let mut messages: Vec<Message> = validator.drain_errors().iter().map(to_message).collect();
        if rc != 0 && !has_errors(&messages) {
            messages.push(Message {
                severity: Severity::Error,
                text: format!("Schema validation failed (libxml2 code {rc})."),
                line: None,
                column: None,
                hint: None,
            });
        }
        Ok(messages)
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
        hint: None,
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

fn file_name_of(path: &Path) -> String {
    path.file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string()
}

fn has_errors(messages: &[Message]) -> bool {
    messages.iter().any(|m| m.severity == Severity::Error)
}

/// Map a libxml StructuredError to our Message.
fn to_message(e: &libxml::error::StructuredError) -> Message {
    use libxml::error::XmlErrorLevel;
    let severity = match e.level {
        XmlErrorLevel::Warning => Severity::Warning,
        _ => Severity::Error,
    };
    let raw = e.message.as_deref().unwrap_or("validation error").trim();
    let text = crate::messages::strip_namespaces(raw);
    let hint = crate::messages::hint_for(&text);
    let line = e.line.filter(|l| *l > 0).map(|l| l as u32);
    let column = e.col.filter(|c| *c > 0).map(|c| c as u32);
    Message {
        severity,
        text,
        line,
        column,
        hint,
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
        let p = temp_named(
            "ch_debtor",
            &pain001("", "CH9300762011623852957", "DE89370400440532013000"),
        );
        assert!(is_swiss(&std::fs::read(&p).unwrap()));
    }

    #[test]
    fn liechtenstein_debtor_iban_is_swiss() {
        let p = temp_named(
            "li_debtor",
            &pain001("", "LI21088100002324013AA", "DE89370400440532013000"),
        );
        assert!(is_swiss(&std::fs::read(&p).unwrap()));
    }

    #[test]
    fn german_debtor_paying_swiss_creditor_is_not_swiss() {
        let p = temp_named(
            "de_debtor",
            &pain001("", "DE89370400440532013000", "CH9300762011623852957"),
        );
        assert!(!is_swiss(&std::fs::read(&p).unwrap()));
    }

    #[test]
    fn swiss_schema_location_is_swiss() {
        let attrs = r#"xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xsi:schemaLocation="urn:iso:std:iso:20022:tech:xsd:pain.001.001.09 pain.001.001.09.ch.03.xsd""#;
        let p = temp_named(
            "ch_location",
            &pain001(attrs, "DE89370400440532013000", "DE89370400440532013000"),
        );
        assert!(is_swiss(&std::fs::read(&p).unwrap()));
    }

    #[test]
    fn swiss_file_resolves_to_swiss_schema() {
        let empty = std::env::temp_dir().join("sepa_swiss_test_empty_schemas");
        std::fs::create_dir_all(&empty).unwrap();
        let mut v = super::Validator::new(empty);

        let ch = temp_named(
            "resolve_ch",
            &pain001("", "CH9300762011623852957", "CH9300762011623852957"),
        );
        let r = v.validate_file(&ch);
        assert_eq!(r.schema, "pain.001.001.09.ch.03.xsd");
        assert_eq!(r.status, Status::NoSchema);

        let de = temp_named(
            "resolve_de",
            &pain001("", "DE89370400440532013000", "CH9300762011623852957"),
        );
        assert_eq!(v.validate_file(&de).schema, "pain.001.001.09.xsd");
    }

    /// pain.001.001.09 with one transaction; `extra` goes in front of its amount.
    fn pain001_tx(debtor_iban: &str, ccy: &str, extra: &str) -> String {
        format!(
            r#"<?xml version="1.0"?><Document xmlns="urn:iso:std:iso:20022:tech:xsd:pain.001.001.09"><CstmrCdtTrfInitn><PmtInf><PmtInfId>P1</PmtInfId><PmtMtd>TRF</PmtMtd><DbtrAcct><Id><IBAN>{debtor_iban}</IBAN></Id></DbtrAcct><CdtTrfTxInf>{extra}<Amt><InstdAmt Ccy="{ccy}">1.00</InstdAmt></Amt></CdtTrfTxInf></PmtInf></CstmrCdtTrfInitn></Document>"#
        )
    }

    const DE_IBAN: &str = "DE89370400440532013000";

    fn kind_of(_tag: &str, xml: &str) -> schema::Variant {
        detect_variant(xml.as_bytes())
    }

    #[test]
    fn sepa_credit_transfers_are_standard() {
        let sepa = "<PmtTpInf><SvcLvl><Cd>SEPA</Cd></SvcLvl></PmtTpInf><ChrgBr>SLEV</ChrgBr>";
        assert_eq!(
            kind_of("sepa", &pain001_tx(DE_IBAN, "EUR", sepa)),
            schema::Variant::Standard
        );
        assert_eq!(
            kind_of("sepa_bare", &pain001_tx(DE_IBAN, "EUR", "")),
            schema::Variant::Standard
        );
    }

    #[test]
    fn urgent_euro_transfers_are_ccu() {
        for (tag, extra) in [
            (
                "urgp",
                "<PmtTpInf><SvcLvl><Cd>URGP</Cd></SvcLvl></PmtTpInf>",
            ),
            ("shar", "<ChrgBr>SHAR</ChrgBr>"),
            (
                "both",
                "<PmtTpInf><SvcLvl><Cd>URGP</Cd></SvcLvl></PmtTpInf><ChrgBr>SHAR</ChrgBr>",
            ),
        ] {
            let xml = pain001_tx(DE_IBAN, "EUR", extra);
            assert_eq!(kind_of(tag, &xml), schema::Variant::Urgent, "{tag}");
        }
    }

    #[test]
    fn foreign_signals_make_a_file_axz() {
        for (tag, ccy, extra) in [
            ("debt", "EUR", "<ChrgBr>DEBT</ChrgBr>"),
            ("cred", "EUR", "<ChrgBr>CRED</ChrgBr>"),
            ("usd", "USD", ""),
            ("usd_shar", "USD", "<ChrgBr>SHAR</ChrgBr>"),
            (
                "nurg",
                "EUR",
                "<PmtTpInf><SvcLvl><Cd>NURG</Cd></SvcLvl></PmtTpInf>",
            ),
            (
                "urgp_debt",
                "EUR",
                "<PmtTpInf><SvcLvl><Cd>URGP</Cd></SvcLvl></PmtTpInf><ChrgBr>DEBT</ChrgBr>",
            ),
            (
                "eqvtamt",
                "EUR",
                r#"<EqvtAmt><Amt Ccy="EUR">1.00</Amt><CcyOfTrf>USD</CcyOfTrf></EqvtAmt>"#,
            ),
        ] {
            let xml = pain001_tx(DE_IBAN, ccy, extra);
            assert_eq!(kind_of(tag, &xml), schema::Variant::Foreign, "{tag}");
        }
        let cheque = pain001_tx(DE_IBAN, "EUR", "").replace(">TRF<", ">CHK<");
        assert_eq!(kind_of("chk", &cheque), schema::Variant::Foreign);
    }

    #[test]
    fn schema_location_names_the_subset() {
        let with_location = |xsd: &str| {
            pain001_tx(DE_IBAN, "EUR", "").replace(
                r#"pain.001.001.09">"#,
                &format!(
                    r#"pain.001.001.09" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xsi:schemaLocation="urn:iso:std:iso:20022:tech:xsd:pain.001.001.09 {xsd}">"#
                ),
            )
        };
        assert_eq!(
            kind_of("loc_axz", &with_location("pain.001.001.09_AXZ_GBIC_5.xsd")),
            schema::Variant::Foreign
        );
        assert_eq!(
            kind_of("loc_ccu", &with_location("pain.001.001.09_CCU_GBIC_5.xsd")),
            schema::Variant::Urgent
        );
    }

    #[test]
    fn swiss_wins_over_dk_signals() {
        let xml = pain001_tx("CH9300762011623852957", "CHF", "<ChrgBr>SHAR</ChrgBr>");
        assert_eq!(kind_of("ch_shar", &xml), schema::Variant::Swiss);
    }

    #[test]
    fn dk_subsets_resolve_to_their_schemas() {
        let empty = std::env::temp_dir().join("sepa_swiss_test_empty_schemas");
        std::fs::create_dir_all(&empty).unwrap();
        let mut v = super::Validator::new(empty);
        for (tag, ccy, extra, schema) in [
            (
                "axz",
                "GBP",
                "<ChrgBr>DEBT</ChrgBr>",
                "pain.001.001.09_AXZ_GBIC_5.xsd",
            ),
            (
                "ccu",
                "EUR",
                "<ChrgBr>SHAR</ChrgBr>",
                "pain.001.001.09_CCU_GBIC_5.xsd",
            ),
            ("sct", "EUR", "<ChrgBr>SLEV</ChrgBr>", "pain.001.001.09.xsd"),
        ] {
            let p = temp_named(&format!("resolve_{tag}"), &pain001_tx(DE_IBAN, ccy, extra));
            let r = v.validate_file(&p);
            assert_eq!(r.schema, schema, "{tag}");
            assert_eq!(r.status, Status::NoSchema, "{tag}");
        }
    }

    /// Self-written mini schemas named like the DK subsets: SEPA and CCU reject
    /// everything, AXZ accepts everything (committed, so CI runs these tests).
    fn mini_subset_dir() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("dk_subsets")
    }

    #[test]
    fn a_failed_guess_falls_back_to_the_dk_subset_that_accepts_the_file() {
        let shar = temp_named(
            "mini_shar",
            &pain001_tx(DE_IBAN, "EUR", "<ChrgBr>SHAR</ChrgBr>"),
        );
        let r = super::Validator::new(mini_subset_dir()).validate_file(&shar);
        assert_eq!(r.schema, "pain.001.001.09_AXZ_GBIC_5.xsd");
        assert_eq!(r.errors, 0, "{:?}", r.messages);
    }

    #[test]
    fn the_guess_is_kept_when_no_dk_subset_accepts_the_file() {
        let dir = std::env::temp_dir().join("sepa_mini_subsets_without_axz");
        std::fs::create_dir_all(&dir).unwrap();
        for f in ["pain.001.001.09.xsd", "pain.001.001.09_CCU_GBIC_5.xsd"] {
            std::fs::copy(mini_subset_dir().join(f), dir.join(f)).unwrap();
        }
        let _ = std::fs::remove_file(dir.join("pain.001.001.09_AXZ_GBIC_5.xsd"));
        let shar = temp_named(
            "mini_shar_kept",
            &pain001_tx(DE_IBAN, "EUR", "<ChrgBr>SHAR</ChrgBr>"),
        );
        let r = super::Validator::new(dir).validate_file(&shar);
        assert_eq!(r.schema, "pain.001.001.09_CCU_GBIC_5.xsd");
        assert_eq!(r.status, Status::Invalid);
    }

    #[test]
    fn swiss_files_never_fall_back_to_dk_subsets() {
        let dir = std::env::temp_dir().join("sepa_mini_subsets_swiss");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::copy(
            mini_subset_dir().join("pain.001.001.09.xsd"),
            dir.join("pain.001.001.09.ch.03.xsd"),
        )
        .unwrap();
        std::fs::copy(
            mini_subset_dir().join("pain.001.001.09_AXZ_GBIC_5.xsd"),
            dir.join("pain.001.001.09_AXZ_GBIC_5.xsd"),
        )
        .unwrap();
        let ch = temp_named("mini_ch", &pain001_tx("CH9300762011623852957", "CHF", ""));
        let r = super::Validator::new(dir).validate_file(&ch);
        assert_eq!(r.schema, "pain.001.001.09.ch.03.xsd");
        assert_eq!(r.status, Status::Invalid);
    }

    /// A complete DK pain.001.001.09 credit transfer; the parameters set what
    /// tells the subsets apart.
    fn dk_credit_transfer(svc_lvl: &str, ccy: &str, chrg_br: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:pain.001.001.09"><CstmrCdtTrfInitn>
<GrpHdr><MsgId>MSG-1</MsgId><CreDtTm>2026-11-03T09:52:40</CreDtTm><NbOfTxs>1</NbOfTxs><CtrlSum>2500.00</CtrlSum><InitgPty><Nm>Muster GmbH</Nm></InitgPty></GrpHdr>
<PmtInf><PmtInfId>PMT-1</PmtInfId><PmtMtd>TRF</PmtMtd><NbOfTxs>1</NbOfTxs><CtrlSum>2500.00</CtrlSum><ReqdExctnDt><Dt>2099-01-02</Dt></ReqdExctnDt>
<Dbtr><Nm>Muster GmbH</Nm><PstlAdr><TwnNm>Berlin</TwnNm><Ctry>DE</Ctry></PstlAdr></Dbtr><DbtrAcct><Id><IBAN>DE89370400440532013000</IBAN></Id><Ccy>EUR</Ccy></DbtrAcct>
<DbtrAgt><FinInstnId><BICFI>COBADEFFXXX</BICFI></FinInstnId></DbtrAgt>
<CdtTrfTxInf><PmtId><EndToEndId>NOTPROVIDED</EndToEndId></PmtId>
<PmtTpInf><SvcLvl><Cd>{svc_lvl}</Cd></SvcLvl></PmtTpInf>
<Amt><InstdAmt Ccy="{ccy}">2500.00</InstdAmt></Amt><ChrgBr>{chrg_br}</ChrgBr>
<CdtrAgt><FinInstnId><BICFI>CHASUS33XXX</BICFI></FinInstnId></CdtrAgt>
<Cdtr><Nm>Example Inc</Nm><PstlAdr><TwnNm>New York</TwnNm><Ctry>US</Ctry></PstlAdr></Cdtr>
<CdtrAcct><Id><IBAN>DE89370400440532013000</IBAN></Id></CdtrAcct>
<RmtInf><Ustrd>Invoice 1</Ustrd></RmtInf></CdtTrfTxInf>
</PmtInf></CstmrCdtTrfInitn></Document>"#
        )
    }

    #[test]
    fn dk_credit_transfers_validate_against_their_subset() {
        let dir = test_schema_dir();
        let needed = [
            "pain.001.001.09.xsd",
            "pain.001.001.09_AXZ_GBIC_5.xsd",
            "pain.001.001.09_CCU_GBIC_5.xsd",
        ];
        if !needed.iter().all(|f| dir.join(f).exists()) {
            eprintln!("SKIP: DK pain.001.001.09 subsets absent from xml_schema/");
            return;
        }
        let mut v = super::Validator::new(dir);
        for (tag, svc, ccy, chrg, schema) in [
            (
                "axz",
                "NURG",
                "USD",
                "DEBT",
                "pain.001.001.09_AXZ_GBIC_5.xsd",
            ),
            (
                "ccu",
                "URGP",
                "EUR",
                "SHAR",
                "pain.001.001.09_CCU_GBIC_5.xsd",
            ),
        ] {
            let p = temp_named(&format!("dk_{tag}"), &dk_credit_transfer(svc, ccy, chrg));
            let r = v.validate_file(&p);
            assert_eq!(r.schema, schema, "{tag}");
            assert_eq!(r.errors, 0, "{tag} schema-valid: {:?}", r.messages);
        }

        // Looks urgent (EUR, URGP, SHAR) but has a non-IBAN creditor account,
        // which only the AXZ subset allows: the guess (CCU) fails, AXZ is used.
        let xml = dk_credit_transfer("URGP", "EUR", "SHAR").replace(
            "<CdtrAcct><Id><IBAN>DE89370400440532013000</IBAN></Id></CdtrAcct>",
            "<CdtrAcct><Id><Othr><Id>123456789</Id></Othr></Id></CdtrAcct>",
        );
        let r = v.validate_file(&temp_named("dk_fallback", &xml));
        assert_eq!(r.schema, "pain.001.001.09_AXZ_GBIC_5.xsd");
        assert_eq!(r.errors, 0, "fallback schema-valid: {:?}", r.messages);
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
        assert_eq!(r.errors, 0, "schema-valid, messages: {:?}", r.messages);
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
            (
                "pain_008_Swiss-DD_Beispiel_1.xml",
                "pain.008.001.02.ch.03.xsd",
            ),
            ("pain_008_Beispiel_1.xml", "pain.008.001.02.chsdd.02.xsd"),
        ] {
            let r = v.validate_file(&examples.join(file));
            assert_eq!(r.schema, schema);
            assert_eq!(r.errors, 0, "{file} must be schema-valid: {:?}", r.messages);
        }
    }

    use crate::model::Status;
    use crate::schema;

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
        assert_eq!(r.errors, 0, "schema-valid, messages: {:?}", r.messages);
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
