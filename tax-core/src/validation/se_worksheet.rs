use rust_decimal::Decimal;

use crate::validation::report::{ValidationIssue, ValidationReport};
use crate::validation::rules::{DecimalFieldRules, max_money, money};
use crate::validation::worksheet::{Validated, Worksheet};

/// Editable fields on the Self-Employment Tax and Deduction Worksheet.
///
/// Ordering follows the order the fields appear on Form 1040-ES, so a
/// [`ValidationReport`] keyed by this enum reports fields top to bottom.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum SeField {
    /// Line 1a: expected income and profits subject to self-employment tax.
    SeIncome,
    /// Line 1b: expected Conservation Reserve Program payments.
    CrpPayments,
    /// Line 6: expected wages subject to social security or tier 1 RRTA tax.
    ExpectedWages,
}

impl SeField {
    /// Returns the text used to identify the field in messages.
    pub fn label(self) -> &'static str {
        match self {
            SeField::SeIncome => "Line 1a (expected SE income)",
            SeField::CrpPayments => "Line 1b (expected CRP payments)",
            SeField::ExpectedWages => "Line 6 (expected wages)",
        }
    }
}

/// Raw field text taken from the form inputs.
#[derive(Clone, Copy, Debug, Default)]
pub struct SeRawInputs<'a> {
    /// Line 1a text.
    pub se_income: &'a str,
    /// Line 1b text.
    pub crp_payments: &'a str,
    /// Line 6 text.
    pub expected_wages: &'a str,
}

impl SeRawInputs<'_> {
    /// Returns `true` when every field is blank.
    pub fn is_blank(&self) -> bool {
        self.se_income.trim().is_empty()
            && self.crp_payments.trim().is_empty()
            && self.expected_wages.trim().is_empty()
    }
}

/// Parsed worksheet inputs, ready for the calculation layer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SeWorksheetInputs {
    /// Line 1a.
    pub se_income: Option<Decimal>,
    /// Line 1b.
    pub crp_payments: Option<Decimal>,
    /// Line 6.
    pub expected_wages: Option<Decimal>,
}

/// Validator for the Self-Employment Tax and Deduction Worksheet.
#[derive(Clone, Copy, Debug)]
pub struct SeWorksheetValidator;

impl Worksheet for SeWorksheetValidator {
    type Field = SeField;
    type Raw<'a> = SeRawInputs<'a>;
    type Inputs = SeWorksheetInputs;

    fn validate(raw: SeRawInputs<'_>) -> Validated<SeWorksheetInputs, SeField> {
        let mut report = ValidationReport::new();

        if raw.is_blank() {
            return Validated {
                inputs: SeWorksheetInputs::default(),
                report,
            };
        }

        // Line 1a is required, so a blank line 1a is reported whenever another
        // field holds a value.
        let se_income = report.record(
            SeField::SeIncome,
            rules_for(SeField::SeIncome, true).evaluate(raw.se_income),
        );
        let crp_payments = report.record(
            SeField::CrpPayments,
            rules_for(SeField::CrpPayments, false).evaluate(raw.crp_payments),
        );
        let expected_wages = report.record(
            SeField::ExpectedWages,
            rules_for(SeField::ExpectedWages, false).evaluate(raw.expected_wages),
        );

        let inputs = SeWorksheetInputs {
            se_income,
            crp_payments,
            expected_wages,
        };

        // Line 1b is part of line 1a on Schedule F, so a larger line 1b means the
        // two entries contradict each other. This is a warning rather than an
        // error because a farm loss can produce a negative line 1a.
        if !report.has_errors()
            && let Some(income) = inputs.se_income
            && let Some(crp) = inputs.crp_payments
            && crp > income
        {
            report.push(
                SeField::CrpPayments,
                ValidationIssue::warning(format!(
                    "Line 1b ({}) is more than line 1a ({}), so line 2 will be negative.",
                    money(crp),
                    money(income)
                )),
            );
        }

        Validated { inputs, report }
    }
}

/// Builds the parsing and range rules for one field.
///
/// Line 1a allows a negative amount because a net loss from self-employment
/// is possible; the worksheet then produces no self-employment tax. Lines 1b
/// and 6 are reported amounts and cannot be negative.
fn rules_for(
    field: SeField,
    required: bool,
) -> DecimalFieldRules {
    let rules = DecimalFieldRules::new(field.label())
        .required(required)
        .max(max_money());

    match field {
        SeField::SeIncome => rules.allow_negative(true).min(-max_money()),
        SeField::CrpPayments | SeField::ExpectedWages => rules,
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use rust_decimal_macros::dec;

    use super::*;
    use crate::validation::report::Severity;

    fn raw<'a>(
        se_income: &'a str,
        crp_payments: &'a str,
        expected_wages: &'a str,
    ) -> SeRawInputs<'a> {
        SeRawInputs {
            se_income,
            crp_payments,
            expected_wages,
        }
    }

    #[test]
    fn blank_worksheet_reports_nothing() {
        let result = SeWorksheetValidator::validate(raw("", "", ""));

        assert_eq!(result.inputs, SeWorksheetInputs::default());
        assert!(result.report.is_empty());
    }

    #[test]
    fn valid_worksheet_parses_every_field() {
        let result = SeWorksheetValidator::validate(raw("100,000.00", "0.00", "50000"));

        assert!(result.is_valid());
        assert_eq!(result.inputs.se_income, Some(dec!(100000.00)));
        assert_eq!(result.inputs.crp_payments, Some(dec!(0.00)));
        assert_eq!(result.inputs.expected_wages, Some(dec!(50000)));
    }

    #[test]
    fn line_1a_is_required_once_any_field_is_filled() {
        let result = SeWorksheetValidator::validate(raw("", "", "50000"));

        let issues = result.report.issues_for(&SeField::SeIncome);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].severity, Severity::Error);
    }

    #[test]
    fn se_income_above_the_money_limit_reports_an_error() {
        let result = SeWorksheetValidator::validate(raw("1000000000.00", "", ""));

        let issues = result.report.issues_for(&SeField::SeIncome);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].severity, Severity::Error);
    }

    #[test]
    fn negative_se_income_is_accepted() {
        let result = SeWorksheetValidator::validate(raw("-5000.00", "", ""));

        assert!(result.is_valid());
        assert_eq!(result.inputs.se_income, Some(dec!(-5000.00)));
    }

    #[test]
    fn negative_crp_payments_report_an_error() {
        let result = SeWorksheetValidator::validate(raw("50000.00", "-1.00", ""));

        let issues = result.report.issues_for(&SeField::CrpPayments);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].severity, Severity::Error);
    }

    #[test]
    fn crp_payments_above_se_income_report_a_warning_not_an_error() {
        let result = SeWorksheetValidator::validate(raw("1000.00", "2000.00", ""));

        let issues = result.report.issues_for(&SeField::CrpPayments);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].severity, Severity::Warning);
        assert_eq!(
            &*issues[0].message,
            "Line 1b ($2000.00) is more than line 1a ($1000.00), so line 2 will be negative."
        );
        assert!(result.is_valid());
    }

    #[test]
    fn crp_payments_equal_to_se_income_are_not_flagged() {
        let result = SeWorksheetValidator::validate(raw("1000.00", "1000.00", ""));

        assert!(result.report.issues_for(&SeField::CrpPayments).is_empty());
    }

    #[test]
    fn blank_crp_payments_are_not_compared_with_se_income() {
        let result = SeWorksheetValidator::validate(raw("-5000.00", "", ""));

        assert!(result.report.issues_for(&SeField::CrpPayments).is_empty());
    }

    #[test]
    fn cross_field_rules_are_skipped_when_a_field_fails_to_parse() {
        let result = SeWorksheetValidator::validate(raw("abc", "2000.00", ""));

        assert_eq!(result.report.issues_for(&SeField::SeIncome).len(), 1);
        assert!(result.report.issues_for(&SeField::CrpPayments).is_empty());
    }

    #[test]
    fn wages_above_the_social_security_maximum_are_accepted() {
        let result = SeWorksheetValidator::validate(raw("50000.00", "", "200000.00"));

        assert!(result.report.is_empty());
        assert_eq!(result.inputs.expected_wages, Some(dec!(200000.00)));
    }
}
