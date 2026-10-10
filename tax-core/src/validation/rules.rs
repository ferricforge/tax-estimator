use std::sync::Arc;

use rust_decimal::Decimal;

use crate::validation::report::ValidationIssue;

/// Most decimal places accepted in a currency field.
const MAX_DECIMAL_PLACES: u32 = 2;

/// Largest amount accepted in a currency field ($999,999,999.99).
///
/// The limit exists to catch transposed digits and stray keystrokes before
/// they reach the calculation layer.
pub fn max_money() -> Decimal {
    Decimal::new(99_999_999_999, 2)
}

/// Formats a decimal as a currency amount for use in messages.
///
/// Negative amounts place the sign before the dollar symbol.
pub fn money(value: Decimal) -> String {
    if value < Decimal::ZERO {
        format!("-${:.2}", value.abs())
    } else {
        format!("${value:.2}")
    }
}

/// Removes currency formatting so the remaining text can be parsed.
///
/// Strips whitespace, thousands separators, underscores, and the dollar sign.
fn sanitize(raw: &str) -> String {
    raw.chars()
        .filter(|character| {
            !character.is_whitespace()
                && *character != ','
                && *character != '$'
                && *character != '_'
        })
        .collect()
}

/// Parsing and range rules for a single currency input.
///
/// The rules are deliberately independent of any widget so they can be run
/// against saved records as well as live form text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecimalFieldRules {
    label: Arc<str>,
    required: bool,
    allow_negative: bool,
    min: Option<Decimal>,
    max: Option<Decimal>,
}

impl DecimalFieldRules {
    /// Creates rules for a currency field: optional, non-negative, at most
    /// two decimal places, with no explicit bounds.
    ///
    /// `label` is used verbatim at the start of each message, so it should
    /// read as a noun phrase, for example `"Line 1a (expected SE income)"`.
    pub fn new(label: impl Into<Arc<str>>) -> Self {
        Self {
            label: label.into(),
            required: false,
            allow_negative: false,
            min: None,
            max: None,
        }
    }

    /// Marks the field as required, producing an error when it is blank.
    pub fn required(
        mut self,
        required: bool,
    ) -> Self {
        self.required = required;
        self
    }

    /// Allows negative amounts, such as a net loss from self-employment.
    pub fn allow_negative(
        mut self,
        allow_negative: bool,
    ) -> Self {
        self.allow_negative = allow_negative;
        self
    }

    /// Sets the inclusive lower bound.
    pub fn min(
        mut self,
        min: Decimal,
    ) -> Self {
        self.min = Some(min);
        self
    }

    /// Sets the inclusive upper bound.
    pub fn max(
        mut self,
        max: Decimal,
    ) -> Self {
        self.max = Some(max);
        self
    }

    /// Parses and checks the raw text.
    ///
    /// Returns `Ok(None)` when the field is blank and optional, `Ok(Some(_))`
    /// for an accepted amount, and `Err(_)` with the first failing rule.
    pub fn evaluate(
        &self,
        raw: &str,
    ) -> Result<Option<Decimal>, ValidationIssue> {
        let cleaned = sanitize(raw);

        if cleaned.is_empty() {
            if self.required {
                return Err(ValidationIssue::error(format!(
                    "{} is required.",
                    self.label
                )));
            }
            return Ok(None);
        }

        let value = Decimal::from_str_exact(&cleaned).map_err(|_| {
            ValidationIssue::error(format!(
                "{} must be an amount, for example 1234.56.",
                self.label
            ))
        })?;

        if value.scale() > MAX_DECIMAL_PLACES {
            return Err(ValidationIssue::error(format!(
                "{} can have at most {} decimal places.",
                self.label, MAX_DECIMAL_PLACES
            )));
        }

        if !self.allow_negative && value < Decimal::ZERO {
            return Err(ValidationIssue::error(format!(
                "{} cannot be negative.",
                self.label
            )));
        }

        if let Some(min) = self.min
            && value < min
        {
            return Err(ValidationIssue::error(format!(
                "{} cannot be less than {}.",
                self.label,
                money(min)
            )));
        }

        if let Some(max) = self.max
            && value > max
        {
            return Err(ValidationIssue::error(format!(
                "{} cannot be more than {}.",
                self.label,
                money(max)
            )));
        }

        Ok(Some(value))
    }
}

/// Presence rules for a single text input.
///
/// Text is trimmed before it is checked and before it is returned, so trailing
/// spaces never reach the calculation or persistence layers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextFieldRules {
    label: Arc<str>,
    required: bool,
}

impl TextFieldRules {
    /// Creates rules for a text field: optional.
    pub fn new(label: impl Into<Arc<str>>) -> Self {
        Self {
            label: label.into(),
            required: false,
        }
    }

    /// Marks the field as required, producing an error when it is blank.
    pub fn required(
        mut self,
        required: bool,
    ) -> Self {
        self.required = required;
        self
    }

    /// Trims and checks the raw text, returning the trimmed value.
    pub fn evaluate(
        &self,
        raw: &str,
    ) -> Result<String, ValidationIssue> {
        let trimmed = raw.trim();

        if trimmed.is_empty() && self.required {
            return Err(ValidationIssue::error(format!(
                "{} is required.",
                self.label
            )));
        }

        Ok(trimmed.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use rust_decimal_macros::dec;

    use super::*;
    use crate::validation::report::Severity;

    fn rules() -> DecimalFieldRules {
        DecimalFieldRules::new("Line 1a (expected SE income)")
    }

    fn text_rules() -> TextFieldRules {
        TextFieldRules::new("Trade name")
    }

    #[test]
    fn blank_optional_field_parses_to_none() {
        let result = rules().evaluate("   ");

        assert_eq!(result, Ok(None));
    }

    #[test]
    fn blank_required_field_reports_an_error() {
        let result = rules().required(true).evaluate("");

        let issue = result.expect_err("blank required field must fail");
        assert_eq!(issue.severity, Severity::Error);
        assert_eq!(&*issue.message, "Line 1a (expected SE income) is required.");
    }

    #[test]
    fn currency_formatting_is_stripped_before_parsing() {
        let result = rules().evaluate(" $1,234.56 ");

        assert_eq!(result, Ok(Some(dec!(1234.56))));
    }

    #[test]
    fn non_numeric_text_reports_an_error() {
        let result = rules().evaluate("twelve");

        assert!(result.is_err());
    }

    #[test]
    fn more_than_two_decimal_places_reports_an_error() {
        let result = rules().evaluate("100.123");

        let issue = result.expect_err("three decimal places must fail");
        assert_eq!(
            &*issue.message,
            "Line 1a (expected SE income) can have at most 2 decimal places."
        );
    }

    #[test]
    fn negative_amount_is_rejected_by_default() {
        let result = rules().evaluate("-1.00");

        let issue = result.expect_err("negative amount must fail");
        assert_eq!(
            &*issue.message,
            "Line 1a (expected SE income) cannot be negative."
        );
    }

    #[test]
    fn negative_amount_is_accepted_when_allowed() {
        let result = rules().allow_negative(true).evaluate("-1500.00");

        assert_eq!(result, Ok(Some(dec!(-1500.00))));
    }

    #[test]
    fn amount_above_the_maximum_reports_an_error() {
        let result = rules().max(dec!(100.00)).evaluate("100.01");

        let issue = result.expect_err("amount above maximum must fail");
        assert_eq!(
            &*issue.message,
            "Line 1a (expected SE income) cannot be more than $100.00."
        );
    }

    #[test]
    fn amount_below_the_minimum_reports_an_error() {
        let result = rules()
            .allow_negative(true)
            .min(dec!(-100.00))
            .evaluate("-100.01");

        let issue = result.expect_err("amount below minimum must fail");
        assert_eq!(
            &*issue.message,
            "Line 1a (expected SE income) cannot be less than -$100.00."
        );
    }

    #[test]
    fn max_money_is_the_documented_limit() {
        assert_eq!(max_money(), dec!(999999999.99));
    }

    #[test]
    fn money_formats_positive_amounts_with_two_places() {
        assert_eq!(money(dec!(5)), "$5.00");
    }

    #[test]
    fn money_places_the_sign_before_the_symbol_for_negative_amounts() {
        assert_eq!(money(dec!(-100)), "-$100.00");
    }

    #[test]
    fn text_is_trimmed_before_it_is_returned() {
        let result = text_rules().evaluate("  Acme Consulting  ");

        assert_eq!(result, Ok("Acme Consulting".to_owned()));
    }

    #[test]
    fn blank_optional_text_returns_an_empty_string() {
        let result = text_rules().evaluate("   ");

        assert_eq!(result, Ok(String::new()));
    }

    #[test]
    fn blank_required_text_reports_an_error() {
        let result = text_rules().required(true).evaluate("");

        let issue = result.expect_err("blank required text must fail");
        assert_eq!(&*issue.message, "Trade name is required.");
    }
}
