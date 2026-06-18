use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub severity: Severity,
    pub text: String,
    pub line: Option<u32>,
    pub column: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Ok,
    Invalid,
    Warnings,
    NoSchema,
    Error,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationResult {
    pub file: String,
    pub path: String,
    pub namespace: String,
    pub schema: String,
    pub status: Status,
    pub errors: u32,
    pub warnings: u32,
    pub messages: Vec<Message>,
}

impl ValidationResult {
    /// Build a result from collected messages, deriving status + counts.
    pub fn from_messages(
        file: String,
        path: String,
        namespace: String,
        schema: String,
        messages: Vec<Message>,
    ) -> Self {
        let errors = messages
            .iter()
            .filter(|m| m.severity == Severity::Error)
            .count() as u32;
        let warnings = messages
            .iter()
            .filter(|m| m.severity == Severity::Warning)
            .count() as u32;
        let status = if errors > 0 {
            Status::Invalid
        } else if warnings > 0 {
            Status::Warnings
        } else {
            Status::Ok
        };
        Self {
            file,
            path,
            namespace,
            schema,
            status,
            errors,
            warnings,
            messages,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_is_ok_with_no_messages() {
        let r = ValidationResult::from_messages(
            "f.xml".into(),
            "/f.xml".into(),
            "ns".into(),
            "s.xsd".into(),
            vec![],
        );
        assert_eq!(r.status, Status::Ok);
        assert_eq!(r.errors, 0);
        assert_eq!(r.warnings, 0);
    }

    #[test]
    fn status_is_invalid_with_an_error() {
        let msgs = vec![Message {
            severity: Severity::Error,
            text: "boom".into(),
            line: Some(4),
            column: None,
        }];
        let r = ValidationResult::from_messages(
            "f.xml".into(),
            "/f.xml".into(),
            "ns".into(),
            "s.xsd".into(),
            msgs,
        );
        assert_eq!(r.status, Status::Invalid);
        assert_eq!(r.errors, 1);
    }

    #[test]
    fn status_is_warnings_when_only_warnings() {
        let msgs = vec![Message {
            severity: Severity::Warning,
            text: "hmm".into(),
            line: None,
            column: None,
        }];
        let r = ValidationResult::from_messages(
            "f.xml".into(),
            "/f.xml".into(),
            "ns".into(),
            "s.xsd".into(),
            msgs,
        );
        assert_eq!(r.status, Status::Warnings);
        assert_eq!(r.warnings, 1);
    }

    #[test]
    fn serializes_status_as_snake_case_string() {
        let r = ValidationResult::from_messages(
            "f.xml".into(),
            "/f.xml".into(),
            "ns".into(),
            "s.xsd".into(),
            vec![],
        );
        let json = serde_json::to_string(&r).unwrap();
        assert!(json.contains("\"status\":\"ok\""));
        assert!(json.contains("\"namespace\":\"ns\""));
    }
}
