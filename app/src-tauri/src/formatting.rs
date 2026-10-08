//! Deterministic XML pretty-printer used for BOTH the code viewer and the
//! validation target, so error line/col numbers match the displayed text.
//!
//! It must not change what the schema sees: only whitespace-only text between
//! tags is re-indented. Leaf text is copied verbatim (leading/trailing spaces
//! are significant for IBAN/BIC patterns and `minLength`), and an empty
//! `<a></a>` stays empty instead of gaining an indented line break.

use std::fmt;
use std::path::Path;

use quick_xml::events::Event;
use quick_xml::reader::Reader;

/// Why a file could not be formatted; `line`/`column` refer to the original file.
#[derive(Debug, Clone, PartialEq)]
pub struct FormatError {
    pub message: String,
    pub line: Option<u32>,
    pub column: Option<u32>,
}

impl fmt::Display for FormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

/// What was last written inside the innermost open element.
#[derive(Clone, Copy, PartialEq)]
enum Last {
    /// Its start tag: no content yet.
    Start,
    /// Text or CDATA content.
    Text,
    /// A child element, comment or anything else.
    Markup,
}

/// Pretty-print the XML at `path` with 2-space indentation.
/// Returns Err if the file isn't readable, UTF-8, or well-formed.
pub fn format_xml(path: &Path) -> Result<String, FormatError> {
    let bytes = std::fs::read(path).map_err(|e| FormatError {
        message: e.to_string(),
        line: None,
        column: None,
    })?;
    let text = std::str::from_utf8(&bytes).map_err(|e| {
        at(
            &bytes,
            e.valid_up_to(),
            "Invalid UTF-8 byte sequence (SEPA files must be UTF-8 encoded).".into(),
        )
    })?;

    let mut reader = Reader::from_str(text);
    let mut out: Vec<u8> = Vec::with_capacity(text.len() + text.len() / 2);
    let mut open: Vec<Vec<u8>> = Vec::new();
    let mut pending_ws: Vec<u8> = Vec::new();
    let mut last = Last::Markup;

    let newline_indent = |out: &mut Vec<u8>, depth: usize| {
        if !out.is_empty() {
            out.push(b'\n');
        }
        out.resize(out.len() + depth * 2, b' ');
    };

    loop {
        let event = reader.read_event().map_err(|e| {
            at(
                &bytes,
                reader.error_position() as usize,
                format!("XML not well-formed: {e}"),
            )
        })?;
        match event {
            Event::Start(e) => {
                pending_ws.clear();
                newline_indent(&mut out, open.len());
                out.push(b'<');
                out.extend_from_slice(&e);
                out.push(b'>');
                open.push(e.name().as_ref().to_vec());
                last = Last::Start;
            }
            Event::Empty(e) => {
                pending_ws.clear();
                newline_indent(&mut out, open.len());
                out.push(b'<');
                out.extend_from_slice(&e);
                out.extend_from_slice(b"/>");
                last = Last::Markup;
            }
            Event::End(e) => {
                open.pop();
                if last == Last::Markup {
                    newline_indent(&mut out, open.len());
                } else {
                    // Leaf element: its content (even whitespace-only) is the value.
                    out.extend_from_slice(&pending_ws);
                }
                pending_ws.clear();
                out.extend_from_slice(b"</");
                out.extend_from_slice(&e);
                out.push(b'>');
                last = Last::Markup;
            }
            Event::Text(t) if t.iter().all(|b| matches!(b, b' ' | b'\t' | b'\r' | b'\n')) => {
                pending_ws.extend_from_slice(&t);
            }
            Event::Text(t) => {
                if open.is_empty() {
                    newline_indent(&mut out, 0); // stray text outside the root: keep it visible
                }
                out.extend_from_slice(&pending_ws);
                pending_ws.clear();
                out.extend_from_slice(&t);
                last = Last::Text;
            }
            Event::CData(c) => {
                out.extend_from_slice(&pending_ws);
                pending_ws.clear();
                out.extend_from_slice(b"<![CDATA[");
                out.extend_from_slice(&c);
                out.extend_from_slice(b"]]>");
                last = Last::Text;
            }
            Event::Comment(c) => {
                write_inline_or_line(
                    &mut out,
                    &mut pending_ws,
                    open.len(),
                    last,
                    &[b"<!--", &c, b"-->"],
                );
            }
            Event::PI(p) => {
                write_inline_or_line(
                    &mut out,
                    &mut pending_ws,
                    open.len(),
                    last,
                    &[b"<?", &p, b"?>"],
                );
            }
            Event::Decl(d) => {
                newline_indent(&mut out, open.len());
                out.extend_from_slice(b"<?");
                out.extend_from_slice(&d);
                out.extend_from_slice(b"?>");
            }
            Event::DocType(_) => {
                return Err(at(
                    &bytes,
                    reader.buffer_position() as usize,
                    "DOCTYPE declarations are not allowed in SEPA files.".into(),
                ));
            }
            Event::Eof => {
                if let Some(name) = open.last() {
                    return Err(at(
                        &bytes,
                        bytes.len(),
                        format!(
                            "Unexpected end of file: element <{}> is never closed (file truncated?).",
                            String::from_utf8_lossy(name)
                        ),
                    ));
                }
                break;
            }
        }
    }
    String::from_utf8(out).map_err(|e| FormatError {
        message: e.to_string(),
        line: None,
        column: None,
    })
}

/// Comments/PIs inside a leaf would change its text value if moved onto their
/// own line, so keep them inline there; elsewhere give them a line.
fn write_inline_or_line(
    out: &mut Vec<u8>,
    pending_ws: &mut Vec<u8>,
    depth: usize,
    last: Last,
    parts: &[&[u8]],
) {
    if depth > 0 && last != Last::Markup {
        out.extend_from_slice(pending_ws);
    } else {
        if !out.is_empty() {
            out.push(b'\n');
        }
        out.resize(out.len() + depth * 2, b' ');
    }
    pending_ws.clear();
    for p in parts {
        out.extend_from_slice(p);
    }
}

/// Build an error located at byte `pos` of the original file.
fn at(bytes: &[u8], pos: usize, message: String) -> FormatError {
    let pos = pos.min(bytes.len());
    let before = &bytes[..pos];
    let line = before.iter().filter(|&&b| b == b'\n').count() + 1;
    let line_start = before
        .iter()
        .rposition(|&b| b == b'\n')
        .map_or(0, |i| i + 1);
    FormatError {
        message,
        line: u32::try_from(line).ok(),
        column: u32::try_from(pos - line_start + 1).ok(),
    }
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
        assert!(
            out.lines().count() > 3,
            "expected multiple lines, got:\n{out}"
        );
        assert!(out.contains("<B>1</B>"));
        assert!(out.contains("<C>2</C>"));
    }

    #[test]
    fn malformed_xml_errors() {
        let p = temp_xml("sepa_fmt_bad.xml", "<a><b></a>");
        assert!(format_xml(&p).is_err());
    }

    #[test]
    fn leaf_text_is_kept_verbatim() {
        let p = temp_xml("sepa_fmt_leaf_ws.xml", "<a><b> x </b><c> </c></a>");
        let out = format_xml(&p).unwrap();
        assert!(out.contains("<b> x </b>"), "got:\n{out}");
        assert!(out.contains("<c> </c>"), "got:\n{out}");
    }

    #[test]
    fn empty_element_pair_stays_empty() {
        let p = temp_xml("sepa_fmt_empty.xml", "<a><b></b></a>");
        let out = format_xml(&p).unwrap();
        assert!(out.contains("<b></b>"), "got:\n{out}");
    }

    #[test]
    fn unclosed_root_errors() {
        let p = temp_xml("sepa_fmt_unclosed.xml", "<a><b>1</b>");
        assert!(format_xml(&p).is_err());
    }

    #[test]
    fn doctype_errors() {
        let p = temp_xml("sepa_fmt_doctype.xml", "<!DOCTYPE a><a/>");
        assert!(format_xml(&p).is_err());
    }

    #[test]
    fn error_carries_line_of_original_file() {
        let p = temp_xml("sepa_fmt_err_line.xml", "<a>\n<b>\n</c>\n</a>");
        let e = format_xml(&p).unwrap_err();
        assert_eq!(e.line, Some(3), "got: {e:?}");
    }
}
