use std::collections::BTreeMap;
use std::sync::Arc;

/// Severity of a single validation message.
///
/// Ordered so that `Warning < Error`, which lets callers sort or compare
/// severities when summarizing a report.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// The value is usable, but the user should be aware of a consequence.
    Warning,
    /// The value cannot be used; calculation and persistence must be blocked.
    Error,
}

/// A single message attached to a field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidationIssue {
    /// Whether the issue blocks calculation or is advisory only.
    pub severity: Severity,
    /// Text describing the problem, suitable for display.
    ///
    /// The text never changes after construction and is handed to a renderer
    /// on every frame, so it is reference counted rather than copied.
    pub message: Arc<str>,
}

impl ValidationIssue {
    /// Creates a blocking issue.
    pub fn error(message: impl Into<Arc<str>>) -> Self {
        Self {
            severity: Severity::Error,
            message: message.into(),
        }
    }

    /// Creates an advisory issue.
    pub fn warning(message: impl Into<Arc<str>>) -> Self {
        Self {
            severity: Severity::Warning,
            message: message.into(),
        }
    }

    /// Returns `true` when the issue blocks calculation.
    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }
}

/// Validation messages collected per field.
///
/// `K` is the caller's field key, normally a small `Copy` enum. Fields are
/// stored in key order so messages can be rendered in the order the fields
/// appear on the form.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidationReport<K: Ord> {
    issues: BTreeMap<K, Vec<ValidationIssue>>,
}

impl<K: Ord> Default for ValidationReport<K> {
    fn default() -> Self {
        Self {
            issues: BTreeMap::new(),
        }
    }
}

impl<K: Ord> ValidationReport<K> {
    /// Creates an empty report.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns `true` when no field carries a message.
    pub fn is_empty(&self) -> bool {
        self.issues.is_empty()
    }

    /// Returns `true` when at least one field carries a blocking issue.
    pub fn has_errors(&self) -> bool {
        self.issues
            .values()
            .flatten()
            .any(ValidationIssue::is_error)
    }

    /// Returns the number of blocking issues across all fields.
    pub fn error_count(&self) -> usize {
        self.issues
            .values()
            .flatten()
            .filter(|issue| issue.is_error())
            .count()
    }

    /// Returns the messages recorded for a field, in insertion order.
    pub fn issues_for(
        &self,
        field: &K,
    ) -> &[ValidationIssue] {
        self.issues
            .get(field)
            .map_or(&[][..], |field_issues| field_issues.as_slice())
    }

    /// Appends a message to a field.
    pub fn push(
        &mut self,
        field: K,
        issue: ValidationIssue,
    ) {
        self.issues.entry(field).or_default().push(issue);
    }

    /// Stores the error from `result` against `field`, and returns the parsed
    /// value, or the default value when the check failed.
    pub fn record<T: Default>(
        &mut self,
        field: K,
        result: Result<T, ValidationIssue>,
    ) -> T {
        match result {
            Ok(value) => value,
            Err(issue) => {
                self.push(field, issue);
                T::default()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
    enum TestField {
        First,
        Second,
    }

    #[test]
    fn new_report_is_empty_and_has_no_errors() {
        let report: ValidationReport<TestField> = ValidationReport::new();

        assert!(report.is_empty());
        assert!(!report.has_errors());
        assert_eq!(report.error_count(), 0);
    }

    #[test]
    fn warnings_do_not_count_as_errors() {
        let mut report = ValidationReport::new();
        report.push(TestField::First, ValidationIssue::warning("advisory"));

        assert!(!report.is_empty());
        assert!(!report.has_errors());
        assert_eq!(report.error_count(), 0);
    }

    #[test]
    fn push_keeps_messages_in_insertion_order() {
        let mut report = ValidationReport::new();
        report.push(TestField::First, ValidationIssue::error("first"));
        report.push(TestField::First, ValidationIssue::warning("second"));

        let issues = report.issues_for(&TestField::First);

        assert_eq!(issues.len(), 2);
        assert_eq!(&*issues[0].message, "first");
        assert_eq!(&*issues[1].message, "second");
    }

    #[test]
    fn record_returns_the_value_when_the_check_passes() {
        let mut report: ValidationReport<TestField> = ValidationReport::new();

        let value = report.record(TestField::First, Ok(5));

        assert_eq!(value, 5);
        assert!(report.is_empty());
    }

    #[test]
    fn record_stores_the_error_and_returns_the_default() {
        let mut report: ValidationReport<TestField> = ValidationReport::new();
        let result: Result<u8, ValidationIssue> = Err(ValidationIssue::error("bad"));

        let value = report.record(TestField::Second, result);

        assert_eq!(value, 0);
        assert!(report.has_errors());
        assert_eq!(report.issues_for(&TestField::Second).len(), 1);
    }
}
