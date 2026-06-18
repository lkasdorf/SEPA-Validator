//! Embedded ISO 20022 / GBIC XSD schemas and namespace lookup.

macro_rules! xsd {
    ($f:literal) => {
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../xml_schema/",
            $f
        ))
    };
}

/// Ordered namespace -> (display filename, embedded bytes).
pub const SCHEMAS: &[(&str, &str, &[u8])] = &[
    (
        "urn:iso:std:iso:20022:tech:xsd:pain.001.001.03",
        "pain.001.001.03.xsd",
        xsd!("pain.001.001.03.xsd"),
    ),
    (
        "urn:iso:std:iso:20022:tech:xsd:pain.001.001.09",
        "pain.001.001.09.xsd",
        xsd!("pain.001.001.09.xsd"),
    ),
    (
        "urn:iso:std:iso:20022:tech:xsd:pain.002.001.10",
        "pain.002.001.10.xsd",
        xsd!("pain.002.001.10.xsd"),
    ),
    (
        "urn:iso:std:iso:20022:tech:xsd:pain.007.001.09",
        "pain.007.001.09.xsd",
        xsd!("pain.007.001.09_GBIC_5.xsd"),
    ),
    (
        "urn:iso:std:iso:20022:tech:xsd:pain.008.001.02",
        "pain.008.001.02.xsd",
        xsd!("pain.008.001.02.xsd"),
    ),
    (
        "urn:iso:std:iso:20022:tech:xsd:pain.008.001.08",
        "pain.008.001.08.xsd",
        xsd!("pain.008.001.08.xsd"),
    ),
    (
        "urn:iso:std:iso:20022:tech:xsd:camt.054.001.08",
        "camt.054.001.08.xsd",
        xsd!("camt.054.001.08.xsd"),
    ),
    (
        "urn:conxml:xsd:container.nnn.001.GBIC4",
        "container.nnn.001.GBIC4.xsd",
        xsd!("container.nnn.001.GBIC4.xsd"),
    ),
];

/// Returns (display_filename, xsd_bytes) for a namespace, if known.
pub fn lookup(namespace: &str) -> Option<(&'static str, &'static [u8])> {
    SCHEMAS
        .iter()
        .find(|(ns, _, _)| *ns == namespace)
        .map(|(_, name, bytes)| (*name, *bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_schema_has_non_empty_bytes() {
        for (ns, name, bytes) in SCHEMAS {
            assert!(!bytes.is_empty(), "{ns} -> {name} embedded empty");
            assert!(name.ends_with(".xsd"));
        }
    }

    #[test]
    fn lookup_known_namespace() {
        let (name, bytes) = lookup("urn:iso:std:iso:20022:tech:xsd:pain.008.001.02").unwrap();
        assert_eq!(name, "pain.008.001.02.xsd");
        assert!(bytes.len() > 100);
    }

    #[test]
    fn lookup_unknown_namespace_is_none() {
        assert!(lookup("urn:made:up").is_none());
    }
}
