//! ISO 20022 / GBIC schema namespace → XSD filename map.
//! Schemas are NOT embedded; they are loaded at runtime from the app's schema
//! directory and imported by the user (legal redistribution constraint).

/// Ordered namespace -> expected XSD filename (looked up in the schema dir).
pub const SCHEMAS: &[(&str, &str)] = &[
    (
        "urn:iso:std:iso:20022:tech:xsd:pain.001.001.03",
        "pain.001.001.03.xsd",
    ),
    (
        "urn:iso:std:iso:20022:tech:xsd:pain.001.001.09",
        "pain.001.001.09.xsd",
    ),
    (
        "urn:iso:std:iso:20022:tech:xsd:pain.002.001.10",
        "pain.002.001.10.xsd",
    ),
    (
        "urn:iso:std:iso:20022:tech:xsd:pain.007.001.09",
        "pain.007.001.09_GBIC_5.xsd",
    ),
    (
        "urn:iso:std:iso:20022:tech:xsd:pain.008.001.02",
        "pain.008.001.02.xsd",
    ),
    (
        "urn:iso:std:iso:20022:tech:xsd:pain.008.001.08",
        "pain.008.001.08.xsd",
    ),
    (
        "urn:iso:std:iso:20022:tech:xsd:camt.054.001.08",
        "camt.054.001.08.xsd",
    ),
    (
        "urn:conxml:xsd:container.nnn.001.GBIC4",
        "container.nnn.001.GBIC4.xsd",
    ),
    // Swiss Payment Standards (SIX) direct debits carry their own namespaces.
    (
        "http://www.six-interbank-clearing.com/de/pain.008.001.02.ch.03.xsd",
        "pain.008.001.02.ch.03.xsd",
    ),
    (
        "http://www.six-interbank-clearing.com/de/pain.008.001.02.chsdd.02.xsd",
        "pain.008.001.02.chsdd.02.xsd",
    ),
];

/// Swiss Payment Standards variants that reuse a standard ISO namespace, so the
/// namespace alone can't select them; the validator decides per file (`resolve`).
pub const SWISS_VARIANTS: &[(&str, &str)] = &[(
    "urn:iso:std:iso:20022:tech:xsd:pain.001.001.09",
    "pain.001.001.09.ch.03.xsd",
)];

/// Returns the standard XSD filename for a namespace, if known.
pub fn lookup(namespace: &str) -> Option<&'static str> {
    find(SCHEMAS, namespace)
}

/// True if the namespace has a Swiss variant that is worth sniffing the file for.
pub fn has_swiss_variant(namespace: &str) -> bool {
    find(SWISS_VARIANTS, namespace).is_some()
}

/// XSD filename for a namespace, preferring the Swiss variant for Swiss files.
pub fn resolve(namespace: &str, swiss: bool) -> Option<&'static str> {
    swiss
        .then(|| find(SWISS_VARIANTS, namespace))
        .flatten()
        .or_else(|| lookup(namespace))
}

/// All known (namespace, filename) pairs, for the schema-status UI.
pub fn known_schemas() -> Vec<(&'static str, &'static str)> {
    SCHEMAS.iter().chain(SWISS_VARIANTS).copied().collect()
}

fn find(table: &[(&str, &'static str)], namespace: &str) -> Option<&'static str> {
    table
        .iter()
        .find(|(ns, _)| *ns == namespace)
        .map(|(_, name)| *name)
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
    fn lookup_swiss_direct_debit_namespaces() {
        assert_eq!(
            lookup("http://www.six-interbank-clearing.com/de/pain.008.001.02.ch.03.xsd"),
            Some("pain.008.001.02.ch.03.xsd")
        );
        assert_eq!(
            lookup("http://www.six-interbank-clearing.com/de/pain.008.001.02.chsdd.02.xsd"),
            Some("pain.008.001.02.chsdd.02.xsd")
        );
    }

    #[test]
    fn resolve_picks_swiss_variant_only_when_swiss() {
        let ns = "urn:iso:std:iso:20022:tech:xsd:pain.001.001.09";
        assert_eq!(resolve(ns, true), Some("pain.001.001.09.ch.03.xsd"));
        assert_eq!(resolve(ns, false), Some("pain.001.001.09.xsd"));
    }

    #[test]
    fn resolve_falls_back_to_standard_without_swiss_variant() {
        let ns = "urn:iso:std:iso:20022:tech:xsd:pain.008.001.02";
        assert_eq!(resolve(ns, true), Some("pain.008.001.02.xsd"));
        assert!(resolve("urn:made:up", true).is_none());
    }

    #[test]
    fn has_swiss_variant_only_for_pain001_09() {
        assert!(has_swiss_variant(
            "urn:iso:std:iso:20022:tech:xsd:pain.001.001.09"
        ));
        assert!(!has_swiss_variant(
            "urn:iso:std:iso:20022:tech:xsd:pain.001.001.03"
        ));
    }

    #[test]
    fn known_schemas_lists_all_with_xsd_filenames() {
        let all = known_schemas();
        assert_eq!(all.len(), 11);
        assert!(all
            .iter()
            .any(|(_, name)| *name == "pain.001.001.09.ch.03.xsd"));
        for (ns, name) in all {
            assert!(!ns.is_empty());
            assert!(name.ends_with(".xsd"));
        }
    }
}
