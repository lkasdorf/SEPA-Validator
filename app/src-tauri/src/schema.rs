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
        "urn:iso:std:iso:20022:tech:xsd:camt.052.001.08",
        "camt.052.001.08.xsd",
    ),
    (
        "urn:iso:std:iso:20022:tech:xsd:camt.053.001.08",
        "camt.053.001.08.xsd",
    ),
    (
        "urn:iso:std:iso:20022:tech:xsd:camt.054.001.08",
        "camt.054.001.08.xsd",
    ),
    (
        "urn:conxml:xsd:container.nnn.001.GBIC4",
        "container.nnn.001.GBIC4.xsd",
    ),
    (
        "urn:conxml:xsd:container.nnn.001.GBIC5",
        "container.nnn.001.GBIC5.xsd",
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

/// Which flavour of a shared namespace a file needs. Some schemas reuse a
/// standard ISO namespace, so the namespace alone can't select them; the
/// validator sniffs the file (`validator::detect_variant`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Variant {
    /// The schema from `SCHEMAS` (for pain.001.001.09: DK SEPA credit transfers).
    Standard,
    /// Swiss Payment Standards (SIX).
    Swiss,
    /// DK foreign payments and cheques (AXZ, Anlage 3 chapter 3.2 of the DFÜ-Abkommen).
    Foreign,
    /// DK urgent euro credit transfers (CCU, Anlage 3 chapter 10).
    Urgent,
}

/// Variants that share a standard namespace, with their XSD filenames.
pub const VARIANTS: &[(Variant, &str, &str)] = &[
    (
        Variant::Swiss,
        "urn:iso:std:iso:20022:tech:xsd:pain.001.001.09",
        "pain.001.001.09.ch.03.xsd",
    ),
    (
        Variant::Foreign,
        "urn:iso:std:iso:20022:tech:xsd:pain.001.001.09",
        "pain.001.001.09_AXZ_GBIC_5.xsd",
    ),
    (
        Variant::Urgent,
        "urn:iso:std:iso:20022:tech:xsd:pain.001.001.09",
        "pain.001.001.09_CCU_GBIC_5.xsd",
    ),
];

/// Returns the standard XSD filename for a namespace, if known.
pub fn lookup(namespace: &str) -> Option<&'static str> {
    find(SCHEMAS, namespace)
}

/// True if the namespace has variants that are worth sniffing the file for.
pub fn has_variants(namespace: &str) -> bool {
    VARIANTS.iter().any(|(_, ns, _)| *ns == namespace)
}

/// XSD filename for a namespace and variant, falling back to the standard one.
pub fn resolve(namespace: &str, variant: Variant) -> Option<&'static str> {
    VARIANTS
        .iter()
        .find(|(v, ns, _)| *v == variant && *ns == namespace)
        .map(|(_, _, name)| *name)
        .or_else(|| lookup(namespace))
}

/// The DK (non-Swiss) schemas a namespace can resolve to: the standard one first,
/// then its DK variants. The validator falls back to these when its guess fails.
pub fn dk_candidates(namespace: &str) -> Vec<&'static str> {
    lookup(namespace)
        .into_iter()
        .chain(
            VARIANTS
                .iter()
                .filter(|(v, ns, _)| *v != Variant::Swiss && *ns == namespace)
                .map(|(_, _, name)| *name),
        )
        .collect()
}

/// All known (namespace, filename) pairs, for the schema-status UI.
pub fn known_schemas() -> Vec<(&'static str, &'static str)> {
    SCHEMAS
        .iter()
        .copied()
        .chain(VARIANTS.iter().map(|(_, ns, name)| (*ns, *name)))
        .collect()
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

    const PAIN001_09: &str = "urn:iso:std:iso:20022:tech:xsd:pain.001.001.09";

    #[test]
    fn resolve_picks_the_requested_variant() {
        assert_eq!(
            resolve(PAIN001_09, Variant::Swiss),
            Some("pain.001.001.09.ch.03.xsd")
        );
        assert_eq!(
            resolve(PAIN001_09, Variant::Foreign),
            Some("pain.001.001.09_AXZ_GBIC_5.xsd")
        );
        assert_eq!(
            resolve(PAIN001_09, Variant::Urgent),
            Some("pain.001.001.09_CCU_GBIC_5.xsd")
        );
        assert_eq!(
            resolve(PAIN001_09, Variant::Standard),
            Some("pain.001.001.09.xsd")
        );
    }

    #[test]
    fn dk_candidates_are_standard_then_dk_variants_without_swiss() {
        assert_eq!(
            dk_candidates(PAIN001_09),
            vec![
                "pain.001.001.09.xsd",
                "pain.001.001.09_AXZ_GBIC_5.xsd",
                "pain.001.001.09_CCU_GBIC_5.xsd"
            ]
        );
        assert_eq!(
            dk_candidates("urn:iso:std:iso:20022:tech:xsd:pain.008.001.02"),
            vec!["pain.008.001.02.xsd"]
        );
    }

    #[test]
    fn resolve_falls_back_to_standard_without_a_variant() {
        let ns = "urn:iso:std:iso:20022:tech:xsd:pain.008.001.02";
        assert_eq!(resolve(ns, Variant::Swiss), Some("pain.008.001.02.xsd"));
        assert_eq!(resolve(ns, Variant::Foreign), Some("pain.008.001.02.xsd"));
        assert!(resolve("urn:made:up", Variant::Swiss).is_none());
    }

    #[test]
    fn has_variants_only_for_pain001_09() {
        assert!(has_variants(PAIN001_09));
        assert!(!has_variants(
            "urn:iso:std:iso:20022:tech:xsd:pain.001.001.03"
        ));
    }

    #[test]
    fn lookup_account_statements() {
        assert_eq!(
            lookup("urn:iso:std:iso:20022:tech:xsd:camt.053.001.08"),
            Some("camt.053.001.08.xsd")
        );
        assert_eq!(
            lookup("urn:iso:std:iso:20022:tech:xsd:camt.052.001.08"),
            Some("camt.052.001.08.xsd")
        );
    }

    #[test]
    fn lookup_container_gbic5() {
        assert_eq!(
            lookup("urn:conxml:xsd:container.nnn.001.GBIC5"),
            Some("container.nnn.001.GBIC5.xsd")
        );
    }

    #[test]
    fn known_schemas_lists_all_with_xsd_filenames() {
        let all = known_schemas();
        assert_eq!(all.len(), 16);
        assert!(all
            .iter()
            .any(|(_, name)| *name == "pain.001.001.09.ch.03.xsd"));
        assert!(all
            .iter()
            .any(|(_, name)| *name == "pain.001.001.09_AXZ_GBIC_5.xsd"));
        for (ns, name) in all {
            assert!(!ns.is_empty());
            assert!(name.ends_with(".xsd"));
        }
    }
}
