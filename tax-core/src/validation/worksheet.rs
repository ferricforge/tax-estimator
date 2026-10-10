//! The contract every worksheet validator implements.
//!
//! A validator takes the text read from a form and returns the parsed values
//! together with the messages for every field. Forms call the same method for
//! every worksheet and read the same result type back.

use crate::validation::report::ValidationReport;

/// A worksheet whose raw form text can be checked and parsed.
pub trait Worksheet {
    /// Keys for the editable fields. Ordering follows the form.
    type Field: Copy + Ord;

    /// Text read from the form for one validation pass.
    type Raw<'a>;

    /// Parsed values, ready for the calculation layer.
    type Inputs;

    /// Checks every field and the rules that span fields.
    ///
    /// A field that fails to parse holds its default value in the result, so
    /// a caller that ignores the report still receives well-formed values.
    fn validate(raw: Self::Raw<'_>) -> Validated<Self::Inputs, Self::Field>;
}

/// The outcome of one validation pass.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Validated<I, K: Ord> {
    /// Parsed values. Text is trimmed, and invalid fields hold their default.
    pub inputs: I,
    /// Messages for every field, including warnings.
    pub report: ValidationReport<K>,
}

impl<I: Default, K: Ord> Default for Validated<I, K> {
    fn default() -> Self {
        Self {
            inputs: I::default(),
            report: ValidationReport::default(),
        }
    }
}

impl<I, K: Ord> Validated<I, K> {
    /// Returns `true` when no field carries a blocking message.
    ///
    /// The check covers every field, including fields the user has not edited.
    pub fn is_valid(&self) -> bool {
        !self.report.has_errors()
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::validation::report::ValidationIssue;

    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
    enum TestField {
        Only,
    }

    #[test]
    fn default_result_is_valid() {
        let result: Validated<u8, TestField> = Validated::default();

        assert!(result.is_valid());
        assert_eq!(result.inputs, 0);
    }

    #[test]
    fn warning_alone_keeps_the_result_valid() {
        let mut report = ValidationReport::new();
        report.push(TestField::Only, ValidationIssue::warning("advisory"));
        let result = Validated {
            inputs: 0_u8,
            report,
        };

        assert!(result.is_valid());
    }

    #[test]
    fn error_makes_the_result_invalid() {
        let mut report = ValidationReport::new();
        report.push(TestField::Only, ValidationIssue::error("bad"));
        let result = Validated {
            inputs: 0_u8,
            report,
        };

        assert!(!result.is_valid());
    }
}
