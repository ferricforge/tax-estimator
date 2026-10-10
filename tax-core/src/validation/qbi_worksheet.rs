//! Validation for Form 8995, the simplified qualified business income deduction.

use rust_decimal::Decimal;

use crate::validation::report::{ValidationIssue, ValidationReport};
use crate::validation::rules::{DecimalFieldRules, TextFieldRules, max_money};
use crate::validation::worksheet::{Validated, Worksheet};

/// Row labels printed beside the line 1 table.
const ROW_LABELS: [&str; 5] = ["i", "ii", "iii", "iv", "v"];

/// Number of digits in an EIN, SSN, or ITIN.
const TAXPAYER_ID_DIGITS: usize = 9;

/// Columns of the line 1 table.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum TradeColumn {
    /// Line 1(a): trade, business, or aggregation name.
    Name,
    /// Line 1(b): taxpayer identification number.
    TaxpayerId,
    /// Line 1(c): qualified business income or loss.
    QbiOrLoss,
}

/// Editable fields on Form 8995.
///
/// The line 1 cells are declared first and carry a row index, so a
/// [`ValidationReport`] keyed by this enum reads down the table and then down
/// the remaining lines.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum QbiField {
    /// One cell of the line 1 table.
    Trade {
        /// Zero-based row index.
        row: usize,
        /// Column within the row.
        column: TradeColumn,
    },
    /// Line 3: qualified business net loss carryforward from the prior year.
    QbiLossCarryforward,
    /// Line 6: qualified REIT dividends and PTP income or loss.
    ReitPtpIncome,
    /// Line 7: qualified REIT dividends and PTP loss carryforward.
    ReitPtpLossCarryforward,
    /// Line 12: net capital gain increased by qualified dividends.
    NetCapitalGain,
}

impl QbiField {
    /// Returns the text used to identify the field in messages.
    pub fn label(self) -> String {
        match self {
            QbiField::Trade { row, column } => trade_label(row, column),
            QbiField::QbiLossCarryforward => "Line 3 (QBI loss carryforward)".to_owned(),
            QbiField::ReitPtpIncome => "Line 6 (REIT dividends and PTP income)".to_owned(),
            QbiField::ReitPtpLossCarryforward => {
                "Line 7 (REIT and PTP loss carryforward)".to_owned()
            }
            QbiField::NetCapitalGain => {
                "Line 12 (net capital gain plus qualified dividends)".to_owned()
            }
        }
    }
}

/// Returns the label printed beside a line 1 row.
fn row_label(row: usize) -> String {
    ROW_LABELS
        .get(row)
        .map_or_else(|| (row + 1).to_string(), |label| (*label).to_owned())
}

/// Builds the message label for one cell of the line 1 table.
fn trade_label(
    row: usize,
    column: TradeColumn,
) -> String {
    let row = row_label(row);

    match column {
        TradeColumn::Name => format!("Line 1(a) (trade or business name), row {row}"),
        TradeColumn::TaxpayerId => {
            format!("Line 1(b) (taxpayer identification number), row {row}")
        }
        TradeColumn::QbiOrLoss => format!("Line 1(c) (qualified business income), row {row}"),
    }
}

/// Raw text for one row of the line 1 table.
#[derive(Clone, Copy, Debug, Default)]
pub struct QbiRawTrade<'a> {
    /// Line 1(a) text.
    pub name: &'a str,
    /// Line 1(b) text.
    pub taxpayer_id: &'a str,
    /// Line 1(c) text.
    pub qbi_or_loss: &'a str,
}

/// Raw field text taken from the form inputs.
#[derive(Clone, Copy, Debug, Default)]
pub struct QbiRawInputs<'a> {
    /// Line 1 rows, in form order.
    pub trades: &'a [QbiRawTrade<'a>],
    /// Line 3 text.
    pub qbi_loss_carryforward: &'a str,
    /// Line 6 text.
    pub reit_ptp_income: &'a str,
    /// Line 7 text.
    pub reit_ptp_loss_carryforward: &'a str,
    /// Line 12 text.
    pub net_capital_gain: &'a str,
}

/// One parsed row of the line 1 table.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct QbiTradeInput {
    /// Line 1(a), trimmed.
    pub name: String,
    /// Line 1(b), trimmed.
    pub taxpayer_id: String,
    /// Line 1(c).
    pub qbi_or_loss: Option<Decimal>,
}

/// Parsed worksheet inputs, ready for the calculation layer.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct QbiWorksheetInputs {
    /// Line 1 rows, in form order.
    pub trades: Vec<QbiTradeInput>,
    /// Line 3.
    pub qbi_loss_carryforward: Option<Decimal>,
    /// Line 6.
    pub reit_ptp_income: Option<Decimal>,
    /// Line 7.
    pub reit_ptp_loss_carryforward: Option<Decimal>,
    /// Line 12.
    pub net_capital_gain: Option<Decimal>,
}

/// Validator for Form 8995.
#[derive(Clone, Copy, Debug)]
pub struct QbiWorksheetValidator;

impl Worksheet for QbiWorksheetValidator {
    type Field = QbiField;
    type Raw<'a> = QbiRawInputs<'a>;
    type Inputs = QbiWorksheetInputs;

    fn validate(raw: QbiRawInputs<'_>) -> Validated<QbiWorksheetInputs, QbiField> {
        let mut report = ValidationReport::new();

        let trades: Vec<QbiTradeInput> = raw
            .trades
            .iter()
            .enumerate()
            .map(|(row, trade)| validate_trade(row, trade, &mut report))
            .collect();

        let qbi_loss_carryforward = report.record(
            QbiField::QbiLossCarryforward,
            validate_amount(QbiField::QbiLossCarryforward, raw.qbi_loss_carryforward),
        );
        let reit_ptp_income = report.record(
            QbiField::ReitPtpIncome,
            validate_amount(QbiField::ReitPtpIncome, raw.reit_ptp_income),
        );
        let reit_ptp_loss_carryforward = report.record(
            QbiField::ReitPtpLossCarryforward,
            validate_amount(
                QbiField::ReitPtpLossCarryforward,
                raw.reit_ptp_loss_carryforward,
            ),
        );
        let net_capital_gain = report.record(
            QbiField::NetCapitalGain,
            validate_amount(QbiField::NetCapitalGain, raw.net_capital_gain),
        );

        let inputs = QbiWorksheetInputs {
            trades,
            qbi_loss_carryforward,
            reit_ptp_income,
            reit_ptp_loss_carryforward,
            net_capital_gain,
        };

        Validated { inputs, report }
    }
}

/// Builds the rules for a currency field.
///
/// Line 12 holds net capital gain increased by qualified dividends, which is
/// never negative. Every other amount on the form can be a loss.
fn amount_rules_for(field: QbiField) -> DecimalFieldRules {
    let rules = DecimalFieldRules::new(field.label()).max(max_money());

    match field {
        QbiField::NetCapitalGain => rules,
        _ => rules.allow_negative(true).min(-max_money()),
    }
}

/// Reports an error when a carryforward line holds a positive amount.
///
/// Lines 3 and 7 are printed in parentheses on the form, so they hold a loss
/// or nothing.
fn carryforward_issue(
    field: QbiField,
    value: Decimal,
) -> Option<ValidationIssue> {
    let is_carryforward = matches!(
        field,
        QbiField::QbiLossCarryforward | QbiField::ReitPtpLossCarryforward
    );

    (is_carryforward && value > Decimal::ZERO).then(|| {
        ValidationIssue::error(format!(
            "{} must be zero or a loss. Enter a loss as a negative amount.",
            field.label()
        ))
    })
}

/// Reports an error when a taxpayer identification number is not nine digits.
///
/// An EIN, SSN, or ITIN is nine digits. Dashes and spaces are ignored.
fn taxpayer_id_issue(
    field: QbiField,
    value: &str,
) -> Option<ValidationIssue> {
    if value.is_empty() {
        return None;
    }

    let digits = value.chars().filter(|c| c.is_ascii_digit()).count();
    let separators_only = value
        .chars()
        .all(|c| c.is_ascii_digit() || c == '-' || c == ' ');

    (digits != TAXPAYER_ID_DIGITS || !separators_only).then(|| {
        ValidationIssue::error(format!(
            "{} must be nine digits, for example 12-3456789 or 123-45-6789.",
            field.label()
        ))
    })
}

/// Parses and checks one currency field.
fn validate_amount(
    field: QbiField,
    raw: &str,
) -> Result<Option<Decimal>, ValidationIssue> {
    let value = amount_rules_for(field).evaluate(raw)?;

    if let Some(amount) = value
        && let Some(issue) = carryforward_issue(field, amount)
    {
        return Err(issue);
    }

    Ok(value)
}

/// Trims and checks one taxpayer identification number.
fn validate_taxpayer_id(
    field: QbiField,
    raw: &str,
) -> Result<String, ValidationIssue> {
    let value = TextFieldRules::new(field.label()).evaluate(raw)?;

    match taxpayer_id_issue(field, &value) {
        Some(issue) => Err(issue),
        None => Ok(value),
    }
}

/// Checks one row of the line 1 table and records any errors against it.
fn validate_trade(
    row: usize,
    raw: &QbiRawTrade<'_>,
    report: &mut ValidationReport<QbiField>,
) -> QbiTradeInput {
    let name_field = QbiField::Trade {
        row,
        column: TradeColumn::Name,
    };
    let taxpayer_id_field = QbiField::Trade {
        row,
        column: TradeColumn::TaxpayerId,
    };
    let qbi_field = QbiField::Trade {
        row,
        column: TradeColumn::QbiOrLoss,
    };

    let name = report.record(
        name_field,
        TextFieldRules::new(name_field.label()).evaluate(raw.name),
    );
    let taxpayer_id = report.record(
        taxpayer_id_field,
        validate_taxpayer_id(taxpayer_id_field, raw.taxpayer_id),
    );
    let qbi_or_loss = report.record(qbi_field, validate_amount(qbi_field, raw.qbi_or_loss));

    // Line 1 reports the trade, business, or aggregation by name, so a row
    // identified only by a number or an amount cannot be reported. A name that
    // failed its own check is not reported a second time.
    if name.is_empty()
        && report.issues_for(&name_field).is_empty()
        && (!taxpayer_id.is_empty() || qbi_or_loss.is_some())
    {
        report.push(
            name_field,
            ValidationIssue::error(format!(
                "{} is required when the row has other entries.",
                name_field.label()
            )),
        );
    }

    QbiTradeInput {
        name,
        taxpayer_id,
        qbi_or_loss,
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use rust_decimal_macros::dec;

    use super::*;
    use crate::validation::report::Severity;

    fn trade<'a>(
        name: &'a str,
        taxpayer_id: &'a str,
        qbi_or_loss: &'a str,
    ) -> QbiRawTrade<'a> {
        QbiRawTrade {
            name,
            taxpayer_id,
            qbi_or_loss,
        }
    }

    fn rows<'a>(trades: &'a [QbiRawTrade<'a>]) -> QbiRawInputs<'a> {
        QbiRawInputs {
            trades,
            ..QbiRawInputs::default()
        }
    }

    fn field(
        row: usize,
        column: TradeColumn,
    ) -> QbiField {
        QbiField::Trade { row, column }
    }

    #[test]
    fn blank_worksheet_reports_nothing() {
        let trades = [trade("", "", "")];

        let result = QbiWorksheetValidator::validate(rows(&trades));

        assert!(result.report.is_empty());
        assert_eq!(result.inputs.trades.len(), 1);
        assert_eq!(result.inputs.trades[0], QbiTradeInput::default());
    }

    #[test]
    fn valid_worksheet_parses_every_row_and_line() {
        let trades = [trade("Acme Consulting", "12-3456789", "50,000.00")];
        let raw = QbiRawInputs {
            trades: &trades,
            qbi_loss_carryforward: "-1000.00",
            reit_ptp_income: "-250.00",
            reit_ptp_loss_carryforward: "0.00",
            net_capital_gain: "2500.00",
        };

        let result = QbiWorksheetValidator::validate(raw);

        assert!(result.is_valid());
        assert_eq!(result.inputs.trades[0].name, "Acme Consulting");
        assert_eq!(result.inputs.trades[0].taxpayer_id, "12-3456789");
        assert_eq!(result.inputs.trades[0].qbi_or_loss, Some(dec!(50000.00)));
        assert_eq!(result.inputs.qbi_loss_carryforward, Some(dec!(-1000.00)));
        assert_eq!(result.inputs.reit_ptp_income, Some(dec!(-250.00)));
        assert_eq!(result.inputs.net_capital_gain, Some(dec!(2500.00)));
    }

    #[test]
    fn names_and_taxpayer_ids_are_returned_trimmed() {
        let trades = [trade("  Me is here ", " 12-3456789 ", "")];

        let result = QbiWorksheetValidator::validate(rows(&trades));

        assert!(result.is_valid());
        assert_eq!(result.inputs.trades[0].name, "Me is here");
        assert_eq!(result.inputs.trades[0].taxpayer_id, "12-3456789");
    }

    #[test]
    fn name_is_required_when_the_row_has_an_amount() {
        let trades = [trade("", "", "1000.00")];

        let result = QbiWorksheetValidator::validate(rows(&trades));

        let issues = result.report.issues_for(&field(0, TradeColumn::Name));
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].severity, Severity::Error);
        assert_eq!(
            &*issues[0].message,
            "Line 1(a) (trade or business name), row i is required when the row has other entries."
        );
    }

    #[test]
    fn name_is_required_when_the_row_has_a_taxpayer_id() {
        let trades = [trade("", "12-3456789", "")];

        let result = QbiWorksheetValidator::validate(rows(&trades));

        assert_eq!(
            result.report.issues_for(&field(0, TradeColumn::Name)).len(),
            1
        );
    }

    #[test]
    fn a_social_security_number_is_accepted_as_the_taxpayer_id() {
        let trades = [trade("Acme Consulting", "123-45-6789", "")];

        let result = QbiWorksheetValidator::validate(rows(&trades));

        assert!(result.report.is_empty());
    }

    #[test]
    fn a_blank_taxpayer_id_is_accepted() {
        let trades = [trade("Aggregation 1", "", "1000.00")];

        let result = QbiWorksheetValidator::validate(rows(&trades));

        assert!(result.report.is_empty());
    }

    #[test]
    fn a_taxpayer_id_with_too_few_digits_reports_an_error() {
        let trades = [trade("Acme Consulting", "12-345", "")];

        let result = QbiWorksheetValidator::validate(rows(&trades));

        let issues = result.report.issues_for(&field(0, TradeColumn::TaxpayerId));
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].severity, Severity::Error);
    }

    #[test]
    fn a_taxpayer_id_with_letters_reports_an_error() {
        let trades = [trade("Acme Consulting", "12-3456789X", "")];

        let result = QbiWorksheetValidator::validate(rows(&trades));

        assert_eq!(
            result
                .report
                .issues_for(&field(0, TradeColumn::TaxpayerId))
                .len(),
            1
        );
    }

    #[test]
    fn a_qbi_loss_is_accepted() {
        let trades = [trade("Acme Consulting", "", "-5000.00")];

        let result = QbiWorksheetValidator::validate(rows(&trades));

        assert!(result.report.is_empty());
        assert_eq!(result.inputs.trades[0].qbi_or_loss, Some(dec!(-5000.00)));
    }

    #[test]
    fn a_positive_qbi_loss_carryforward_reports_an_error() {
        let raw = QbiRawInputs {
            qbi_loss_carryforward: "1000.00",
            ..QbiRawInputs::default()
        };

        let result = QbiWorksheetValidator::validate(raw);

        let issues = result.report.issues_for(&QbiField::QbiLossCarryforward);
        assert_eq!(issues.len(), 1);
        assert_eq!(
            &*issues[0].message,
            "Line 3 (QBI loss carryforward) must be zero or a loss. Enter a loss as a negative amount."
        );
    }

    #[test]
    fn a_positive_reit_and_ptp_carryforward_reports_an_error() {
        let raw = QbiRawInputs {
            reit_ptp_loss_carryforward: "0.01",
            ..QbiRawInputs::default()
        };

        let result = QbiWorksheetValidator::validate(raw);

        assert_eq!(
            result
                .report
                .issues_for(&QbiField::ReitPtpLossCarryforward)
                .len(),
            1
        );
    }

    #[test]
    fn a_negative_reit_and_ptp_income_is_accepted() {
        let raw = QbiRawInputs {
            reit_ptp_income: "-500.00",
            ..QbiRawInputs::default()
        };

        let result = QbiWorksheetValidator::validate(raw);

        assert!(result.is_valid());
        assert_eq!(result.inputs.reit_ptp_income, Some(dec!(-500.00)));
    }

    #[test]
    fn a_negative_net_capital_gain_reports_an_error() {
        let raw = QbiRawInputs {
            net_capital_gain: "-1.00",
            ..QbiRawInputs::default()
        };

        let result = QbiWorksheetValidator::validate(raw);

        let issues = result.report.issues_for(&QbiField::NetCapitalGain);
        assert_eq!(issues.len(), 1);
        assert_eq!(
            &*issues[0].message,
            "Line 12 (net capital gain plus qualified dividends) cannot be negative."
        );
    }

    #[test]
    fn an_amount_above_the_money_limit_reports_an_error() {
        let trades = [trade("Acme Consulting", "", "1000000000.00")];

        let result = QbiWorksheetValidator::validate(rows(&trades));

        assert_eq!(
            result
                .report
                .issues_for(&field(0, TradeColumn::QbiOrLoss))
                .len(),
            1
        );
    }

    #[test]
    fn fields_sort_by_row_then_by_column_then_by_line() {
        assert!(field(0, TradeColumn::Name) < field(0, TradeColumn::TaxpayerId));
        assert!(field(0, TradeColumn::QbiOrLoss) < field(1, TradeColumn::Name));
        assert!(field(1, TradeColumn::Name) < QbiField::QbiLossCarryforward);
        assert!(QbiField::QbiLossCarryforward < QbiField::NetCapitalGain);
    }
}
