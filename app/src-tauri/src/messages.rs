//! Turns libxml's raw schema messages into text a person can act on.

/// Remove the `{namespace-uri}` libxml puts in front of every element name.
/// Braces holding anything else (e.g. an enumeration set `{'A', 'B'}`) stay.
pub fn strip_namespaces(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        match after.find('}') {
            Some(close) if is_namespace(&after[..close]) => rest = &after[close + 1..],
            _ => {
                out.push('{');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

fn is_namespace(s: &str) -> bool {
    s.contains(':') && !s.contains(['\'', '"']) && !s.chars().any(char::is_whitespace)
}

/// A plain-language hint for the common SEPA mistakes, given a message that
/// already went through `strip_namespaces`.
pub fn hint_for(text: &str) -> Option<String> {
    let element = between(text, "Element '", "'");
    let tag = |name: &str| format!("<{name}>");
    let e = element.map(tag).unwrap_or_else(|| "This element".into());

    if let Some(value) = offending_value(text) {
        if !value.trim().is_empty() && value.trim() != value {
            return Some("The value has leading or trailing spaces; remove them.".into());
        }
    }
    if text.contains("Missing child element(s).") {
        let expected = expected_elements(text)?;
        return Some(match expected.as_slice() {
            [one] => format!("Required element {} is missing in {e}.", tag(one)),
            many => format!("One of {} is missing in {e}.", list(many)),
        });
    }
    if text.contains("This element is not expected.") {
        return Some(match expected_elements(text).as_deref() {
            Some([one]) => format!("{e} is not allowed here; expected {} (check order and spelling).", tag(one)),
            Some(many) => format!(
                "{e} is not allowed here; expected one of {} (check order and spelling).",
                list(many)
            ),
            None => format!("{e} is not allowed here (unknown element, wrong position, or it occurs too often)."),
        });
    }
    if text.contains("[facet 'enumeration']") {
        let value = offending_value(text)?;
        let set = between(text, "of the set {", "}")?.replace('\'', "");
        return Some(format!(
            "'{value}' is not an allowed code; use one of {set}."
        ));
    }
    if text.contains("[facet 'minLength']") && offending_value(text) == Some("") {
        return Some(format!("{e} must not be empty."));
    }
    if text.contains("[facet 'maxLength']") {
        let len = between(text, "has a length of '", "'")?;
        let max = between(text, "maximum length of '", "'")?;
        return Some(format!(
            "{e} is too long: {len} characters, at most {max} allowed."
        ));
    }
    if text.contains("is not a valid value of the atomic type") {
        let value = offending_value(text)?;
        let numeric = value
            .chars()
            .all(|c| c.is_ascii_digit() || c == ',' || c == '.');
        if numeric && value.contains(',') {
            return Some(format!(
                "Use a dot as the decimal separator, e.g. {}.",
                value.replace('.', "").replace(',', ".")
            ));
        }
    }
    None
}

/// The value libxml quotes as wrong. Values may contain apostrophes, so the end
/// is found by the text that follows the value rather than the next quote.
fn offending_value(text: &str) -> Option<&str> {
    for (start, ends) in [
        ("The value '", &["' is not", "' has a length"][..]),
        ("': '", &["' is not a valid value"][..]),
    ] {
        if let Some(i) = text.find(start) {
            let rest = &text[i + start.len()..];
            if let Some(end) = ends.iter().filter_map(|e| rest.find(e)).min() {
                return Some(&rest[..end]);
            }
        }
    }
    None
}

fn expected_elements(text: &str) -> Option<Vec<&str>> {
    let inner = between(text, "Expected is one of ( ", " )")
        .or_else(|| between(text, "Expected is ( ", " )"))?;
    Some(inner.split(", ").map(str::trim).collect())
}

fn between<'a>(text: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let rest = &text[text.find(start)? + start.len()..];
    Some(&rest[..rest.find(end)?])
}

fn list(names: &[&str]) -> String {
    names
        .iter()
        .map(|n| format!("<{n}>"))
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    const NS: &str = "{urn:iso:std:iso:20022:tech:xsd:pain.001.001.09}";

    #[test]
    fn namespaces_are_removed_from_element_names() {
        let raw = format!("Element '{NS}CtrlSum': '1250,00' is not a valid value of the atomic type 'DecimalNumber'.");
        assert_eq!(
            strip_namespaces(&raw),
            "Element 'CtrlSum': '1250,00' is not a valid value of the atomic type 'DecimalNumber'."
        );
    }

    #[test]
    fn enumeration_sets_are_kept() {
        let raw = "Element 'Cd': [facet 'enumeration'] The value 'XYZ' is not an element of the set {'CORE', 'B2B'}.";
        assert_eq!(strip_namespaces(raw), raw);
    }

    fn hint(raw: &str) -> Option<String> {
        hint_for(&strip_namespaces(raw))
    }

    #[test]
    fn hints_a_missing_element() {
        let raw = format!(
            "Element '{NS}GrpHdr': Missing child element(s). Expected is ( {NS}InitgPty )."
        );
        assert_eq!(
            hint(&raw).as_deref(),
            Some("Required element <InitgPty> is missing in <GrpHdr>.")
        );
    }

    #[test]
    fn hints_an_unexpected_element() {
        let raw = format!("Element '{NS}ReqdExctnDt': This element is not expected. Expected is ( {NS}PmtTpInf ).");
        assert_eq!(
            hint(&raw).as_deref(),
            Some("<ReqdExctnDt> is not allowed here; expected <PmtTpInf> (check order and spelling).")
        );
        let one_of = format!(
            "Element '{NS}Nm': This element is not expected. Expected is one of ( {NS}UltmtCdtr, {NS}Purp, {NS}RmtInf )."
        );
        assert_eq!(
            hint(&one_of).as_deref(),
            Some("<Nm> is not allowed here; expected one of <UltmtCdtr>, <Purp>, <RmtInf> (check order and spelling).")
        );
        let bare = format!("Element '{NS}Ustrd': This element is not expected.");
        assert_eq!(
            hint(&bare).as_deref(),
            Some("<Ustrd> is not allowed here (unknown element, wrong position, or it occurs too often).")
        );
    }

    #[test]
    fn hints_a_code_outside_the_allowed_list() {
        let raw = "Element 'SeqTp': [facet 'enumeration'] The value 'RCURR' is not an element of the set {'FRST', 'RCUR', 'FNAL', 'OOFF'}.";
        assert_eq!(
            hint(raw).as_deref(),
            Some("'RCURR' is not an allowed code; use one of FRST, RCUR, FNAL, OOFF.")
        );
    }

    #[test]
    fn hints_a_decimal_comma() {
        let raw = format!("Element '{NS}CtrlSum': '1250,00' is not a valid value of the atomic type 'DecimalNumber'.");
        assert_eq!(
            hint(&raw).as_deref(),
            Some("Use a dot as the decimal separator, e.g. 1250.00.")
        );
    }

    #[test]
    fn hints_surrounding_spaces() {
        let raw = format!(
            "Element '{NS}IBAN': [facet 'pattern'] The value ' DE89370400440532013000' is not accepted by the pattern '[A-Z]{{2,2}}[0-9]{{2,2}}[a-zA-Z0-9]{{1,30}}'."
        );
        assert_eq!(
            hint(&raw).as_deref(),
            Some("The value has leading or trailing spaces; remove them.")
        );
    }

    #[test]
    fn hints_an_empty_or_too_long_value() {
        let empty = "Element 'Nm': [facet 'minLength'] The value '' has a length of '0'; this underruns the allowed minimum length of '1'.";
        assert_eq!(hint(empty).as_deref(), Some("<Nm> must not be empty."));
        let long = "Element 'Nm': [facet 'maxLength'] The value 'x' has a length of '82'; this exceeds the allowed maximum length of '70'.";
        assert_eq!(
            hint(long).as_deref(),
            Some("<Nm> is too long: 82 characters, at most 70 allowed.")
        );
    }

    #[test]
    fn no_hint_for_unknown_messages() {
        assert_eq!(hint("Something else entirely."), None);
    }
}
