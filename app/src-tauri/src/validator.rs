use std::collections::HashMap;
use std::path::{Path, PathBuf};

use libxml::parser::Parser;
use libxml::schemas::{SchemaParserContext, SchemaValidationContext};
use quick_xml::events::Event;
use quick_xml::name::ResolveResult;
use quick_xml::reader::NsReader;

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

        let schema_name = match schema::lookup(&ns) {
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
            match self.compile(&ns) {
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
                        text: format!("XML parse error: {e}"),
                        line: None,
                        column: None,
                    }],
                    Status::Error,
                    1,
                    0,
                )
            }
        };
        let doc = match Parser::default().parse_string(&formatted) {
            Ok(d) => d,
            Err(e) => {
                return mk(
                    ns.clone(),
                    schema_name.to_string(),
                    vec![Message {
                        severity: Severity::Error,
                        text: format!("XML parse error: {e:?}"),
                        line: None,
                        column: None,
                    }],
                    Status::Error,
                    1,
                    0,
                )
            }
        };

        let messages = match validator.validate_document(&doc) {
            Ok(()) => Vec::new(),
            Err(errors) => errors.iter().map(to_message).collect(),
        };

        ValidationResult::from_messages(file, path_str, ns, schema_name.to_string(), messages)
    }

    fn compile(&self, namespace: &str) -> Result<SchemaValidationContext, String> {
        let filename = schema::lookup(namespace).ok_or("schema not found")?;
        let path = self.schema_dir.join(filename);
        let path_str = path.to_str().ok_or("schema path is not valid UTF-8")?;
        let mut parser = SchemaParserContext::from_file(path_str);
        SchemaValidationContext::from_parser(&mut parser)
            .map_err(|errs| format!("Failed to load schema: {} error(s)", errs.len()))
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
