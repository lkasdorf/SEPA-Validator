use std::path::Path;

use quick_xml::events::Event;
use quick_xml::name::ResolveResult;
use quick_xml::reader::NsReader;

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
        let p = temp_xml(r#"<?xml version="1.0"?><Document xmlns="urn:iso:std:iso:20022:tech:xsd:pain.008.001.02"><X/></Document>"#);
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
}
