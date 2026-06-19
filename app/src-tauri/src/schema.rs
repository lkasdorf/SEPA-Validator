//! ISO 20022 / GBIC schema namespace → XSD filename map.
//! Schemas are NOT embedded; they are loaded at runtime from the app's schema
//! directory and imported by the user (legal redistribution constraint).

/// Ordered namespace -> expected XSD filename (looked up in the schema dir).
pub const SCHEMAS: &[(&str, &str)] = &[
    ("urn:iso:std:iso:20022:tech:xsd:pain.001.001.03", "pain.001.001.03.xsd"),
    ("urn:iso:std:iso:20022:tech:xsd:pain.001.001.09", "pain.001.001.09.xsd"),
    ("urn:iso:std:iso:20022:tech:xsd:pain.002.001.10", "pain.002.001.10.xsd"),
    ("urn:iso:std:iso:20022:tech:xsd:pain.007.001.09", "pain.007.001.09_GBIC_5.xsd"),
    ("urn:iso:std:iso:20022:tech:xsd:pain.008.001.02", "pain.008.001.02.xsd"),
    ("urn:iso:std:iso:20022:tech:xsd:pain.008.001.08", "pain.008.001.08.xsd"),
    ("urn:iso:std:iso:20022:tech:xsd:camt.054.001.08", "camt.054.001.08.xsd"),
    ("urn:conxml:xsd:container.nnn.001.GBIC4", "container.nnn.001.GBIC4.xsd"),
];

/// Returns the XSD filename for a namespace, if known.
pub fn lookup(namespace: &str) -> Option<&'static str> {
    SCHEMAS.iter().find(|(ns, _)| *ns == namespace).map(|(_, name)| *name)
}

/// All known (namespace, filename) pairs, for the schema-status UI.
pub fn known_schemas() -> &'static [(&'static str, &'static str)] {
    SCHEMAS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_known_namespace_returns_filename() {
        assert_eq!(
            lookup("urn:iso:std:iso:20022:tech:xsd:pain.008.001.02"),
            Some("pain.008.001.02.xsd")
        );
    }

    #[test]
    fn lookup_unknown_namespace_is_none() {
        assert!(lookup("urn:made:up").is_none());
    }

    #[test]
    fn known_schemas_lists_all_with_xsd_filenames() {
        let all = known_schemas();
        assert_eq!(all.len(), 8);
        for (ns, name) in all {
            assert!(!ns.is_empty());
            assert!(name.ends_with(".xsd"));
        }
    }
}
