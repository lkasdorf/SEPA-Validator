//! Deterministic XML pretty-printer used for BOTH the code viewer and the
//! validation target, so error line/col numbers match the displayed text.

use std::path::Path;

use quick_xml::events::Event;
use quick_xml::reader::Reader;
use quick_xml::writer::Writer;

/// Pretty-print the XML at `path` with 2-space indentation.
/// Returns Err (with a message) if the file isn't readable or well-formed.
pub fn format_xml(path: &Path) -> Result<String, String> {
    let mut reader = Reader::from_file(path).map_err(|e| e.to_string())?;
    reader.config_mut().trim_text(true);

    let mut writer = Writer::new_with_indent(Vec::new(), b' ', 2);
    let mut buf = Vec::new();
    loop {
        let event = reader.read_event_into(&mut buf).map_err(|e| e.to_string())?;
        if matches!(event, Event::Eof) {
            break;
        }
        writer.write_event(event).map_err(|e| e.to_string())?;
        buf.clear();
    }
    String::from_utf8(writer.into_inner()).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn temp_xml(name: &str, contents: &str) -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        p.push(name);
        let mut f = std::fs::File::create(&p).unwrap();
        f.write_all(contents.as_bytes()).unwrap();
        p
    }

    #[test]
    fn single_line_becomes_multiline() {
        let p = temp_xml(
            "sepa_fmt_single.xml",
            r#"<?xml version="1.0"?><Document xmlns="urn:x"><A><B>1</B><C>2</C></A></Document>"#,
        );
        let out = format_xml(&p).unwrap();
        assert!(out.lines().count() > 3, "expected multiple lines, got:\n{out}");
        assert!(out.contains("<B>1</B>"));
        assert!(out.contains("<C>2</C>"));
    }

    #[test]
    fn malformed_xml_errors() {
        let p = temp_xml("sepa_fmt_bad.xml", "<a><b></a>");
        assert!(format_xml(&p).is_err());
    }
}
