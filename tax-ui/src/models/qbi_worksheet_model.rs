//! Form 8995 "Qualified Business Income Deduction Simplified Computation"
//! (lines 1 through 17).
//!
//! Field names follow the line numbers on the form. Calculations are done by
//! [`tax_core::calculations::QbiWorksheet`]; this model holds the values the
//! user enters, the values that come from the estimate, and the computed
//! lines for display.

use std::fmt;

use rust_decimal::Decimal;
use tax_core::calculations::{
    QBI_DEDUCTION_RATE, QbiWorksheetConfig, QbiWorksheetInput, QbiWorksheetResult,
    qbi_taxable_income_threshold,
};
use tax_core::{Qbi, QbiBusiness, QbiComputed, QbiInput};

use crate::utils::opt_decimal_display;

/// Number of trade, business, or aggregation rows on line 1.
pub const QBI_TRADE_ROW_COUNT: usize = 5;

/// Row labels for the trade, business, or aggregation rows on line 1.
pub const QBI_TRADE_ROW_LABELS: [&str; QBI_TRADE_ROW_COUNT] = ["i", "ii", "iii", "iv", "v"];

/// One row of the line 1 table on Form 8995.
///
/// [`fmt::Debug`] is implemented by hand so the taxpayer identification
/// number is masked in log output.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct QbiTradeEntry {
    /// Column (a): trade, business, or aggregation name.
    ///
    /// For aggregated trades or businesses, the aggregation group name (for
    /// example, Aggregation 1) instead of the business name.
    pub name: String,
    /// Column (b): taxpayer identification number.
    ///
    /// The employer identification number (EIN), or the social security
    /// number (SSN) or individual taxpayer identification number (ITIN) when
    /// there is no EIN. Left blank for an aggregation.
    pub taxpayer_id: String,
    /// Column (c): qualified business income or (loss).
    ///
    /// The net QBI or (loss) for the trade, business, or aggregation. Losses
    /// are negative numbers.
    pub qbi_or_loss: Option<Decimal>,
}

impl fmt::Debug for QbiTradeEntry {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        f.debug_struct("QbiTradeEntry")
            .field("name", &self.name)
            .field("taxpayer_id", &mask_taxpayer_id(&self.taxpayer_id))
            .field("qbi_or_loss", &self.qbi_or_loss)
            .finish()
    }
}

impl QbiTradeEntry {
    /// Returns `true` when every column is blank.
    pub fn is_blank(&self) -> bool {
        self.name.trim().is_empty()
            && self.taxpayer_id.trim().is_empty()
            && self.qbi_or_loss.is_none()
    }
}

/// Form 8995 "Qualified Business Income Deduction Simplified Computation".
///
/// Income is a positive number and a loss is a negative number, including the
/// loss carryforwards on lines 3 and 7. `None` means the line is blank or
/// cannot be figured yet.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct QbiWorksheetModel {
    /// Line 1: trades, businesses, or aggregations (rows i through v).
    pub line_1_trades_or_businesses: [QbiTradeEntry; QBI_TRADE_ROW_COUNT],
    /// Line 2: Total qualified business income or (loss). Combine lines 1i
    /// through 1v, column (c).
    pub line_2_total_qbi_or_loss: Option<Decimal>,
    /// Line 3: Qualified business net (loss) carryforward from the prior year.
    pub line_3_qbi_loss_carryforward: Option<Decimal>,
    /// Line 4: Total qualified business income. Combine lines 2 and 3. If
    /// zero or less, enter -0-.
    pub line_4_total_qbi: Option<Decimal>,
    /// Line 5: Qualified business income component. Multiply line 4 by 20%
    /// (0.20).
    pub line_5_qbi_component: Option<Decimal>,
    /// Line 6: Qualified REIT dividends and publicly traded partnership (PTP)
    /// income or (loss).
    pub line_6_reit_ptp_income: Option<Decimal>,
    /// Line 7: Qualified REIT dividends and qualified PTP (loss) carryforward
    /// from the prior year.
    pub line_7_reit_ptp_loss_carryforward: Option<Decimal>,
    /// Line 8: Total qualified REIT dividends and PTP income. Combine lines 6
    /// and 7. If zero or less, enter -0-.
    pub line_8_total_reit_ptp_income: Option<Decimal>,
    /// Line 9: REIT and PTP component. Multiply line 8 by 20% (0.20).
    pub line_9_reit_ptp_component: Option<Decimal>,
    /// Line 10: Qualified business income deduction before the income
    /// limitation. Add lines 5 and 9.
    pub line_10_deduction_before_income_limitation: Option<Decimal>,
    /// Line 11: Taxable income before qualified business income deduction.
    ///
    /// Comes from the estimate rather than being entered on this form.
    pub line_11_taxable_income_before_qbi: Option<Decimal>,
    /// Line 12: Net capital gain, if any, increased by any qualified
    /// dividends.
    pub line_12_net_capital_gain: Option<Decimal>,
    /// Line 13: Subtract line 12 from line 11. If zero or less, enter -0-.
    pub line_13_taxable_income_less_net_capital_gain: Option<Decimal>,
    /// Line 14: Income limitation. Multiply line 13 by 20% (0.20).
    pub line_14_income_limitation: Option<Decimal>,
    /// Line 15: Qualified business income deduction. The smaller of line 10
    /// or line 14. Also entered on Form 1040 or 1040-SR, line 13a.
    pub line_15_qbi_deduction: Option<Decimal>,
    /// Line 16: Total qualified business (loss) carryforward. Combine lines 2
    /// and 3. If greater than zero, enter -0-.
    pub line_16_total_qbi_loss_carryforward: Option<Decimal>,
    /// Line 17: Total qualified REIT dividends and PTP (loss) carryforward.
    /// Combine lines 6 and 7. If greater than zero, enter -0-.
    pub line_17_total_reit_ptp_loss_carryforward: Option<Decimal>,
    /// Tax year of the estimate this form belongs to.
    pub tax_year: Option<i32>,
    /// Highest taxable income before the QBI deduction for which Form 8995
    /// can be used. `None` when no threshold is known for the tax year.
    pub taxable_income_threshold: Option<Decimal>,
    /// `true` when line 11 is above the threshold, so Form 8995-A must be
    /// used instead.
    pub above_threshold: bool,
}

impl QbiWorksheetModel {
    /// Sets the tax year, the threshold for the filing status, and line 11
    /// from the estimate.
    ///
    /// The threshold is `None` when the tax year is not known, or when no
    /// threshold is known for the year.
    pub fn set_filing_context(
        &mut self,
        tax_year: Option<i32>,
        is_joint: bool,
        taxable_income_before_qbi: Option<Decimal>,
    ) {
        self.tax_year = tax_year;
        self.taxable_income_threshold =
            tax_year.and_then(|year| qbi_taxable_income_threshold(year, is_joint));
        self.line_11_taxable_income_before_qbi = taxable_income_before_qbi;
    }

    /// Builds the worksheet configuration from the model's threshold.
    pub fn worksheet_config(&self) -> QbiWorksheetConfig {
        QbiWorksheetConfig {
            deduction_rate: QBI_DEDUCTION_RATE,
            taxable_income_threshold: self.taxable_income_threshold,
        }
    }

    /// Builds the worksheet input from the entered lines. Blank amounts count
    /// as zero; a blank line 11 stays blank.
    pub fn to_worksheet_input(&self) -> QbiWorksheetInput {
        QbiWorksheetInput {
            trade_or_business_income: self
                .line_1_trades_or_businesses
                .iter()
                .filter_map(|trade| trade.qbi_or_loss)
                .collect(),
            qbi_loss_carryforward: self.line_3_qbi_loss_carryforward.unwrap_or_default(),
            reit_ptp_income: self.line_6_reit_ptp_income.unwrap_or_default(),
            reit_ptp_loss_carryforward: self.line_7_reit_ptp_loss_carryforward.unwrap_or_default(),
            taxable_income_before_qbi: self.line_11_taxable_income_before_qbi,
            net_capital_gain: self.line_12_net_capital_gain.unwrap_or_default(),
        }
    }

    /// Copies the computed lines and the threshold flag from a worksheet
    /// result. The entered lines are left unchanged.
    pub fn from_worksheet_result(
        &mut self,
        result: &QbiWorksheetResult,
    ) {
        self.line_2_total_qbi_or_loss = Some(result.total_qbi_or_loss);
        self.line_4_total_qbi = Some(result.total_qbi);
        self.line_5_qbi_component = Some(result.qbi_component);
        self.line_8_total_reit_ptp_income = Some(result.total_reit_ptp_income);
        self.line_9_reit_ptp_component = Some(result.reit_ptp_component);
        self.line_10_deduction_before_income_limitation =
            Some(result.deduction_before_income_limitation);
        self.line_11_taxable_income_before_qbi = result.taxable_income_before_qbi;
        self.line_13_taxable_income_less_net_capital_gain =
            result.taxable_income_less_net_capital_gain;
        self.line_14_income_limitation = result.income_limitation;
        self.line_15_qbi_deduction = Some(result.qbi_deduction);
        self.line_16_total_qbi_loss_carryforward = Some(result.total_qbi_loss_carryforward);
        self.line_17_total_reit_ptp_loss_carryforward =
            Some(result.total_reit_ptp_loss_carryforward);
        self.above_threshold = result.above_threshold;
    }

    /// Empties every entered and computed line. Keeps the tax year, the
    /// threshold, and line 11, which come from the estimate.
    pub fn clear_entries(&mut self) {
        *self = Self {
            tax_year: self.tax_year,
            taxable_income_threshold: self.taxable_income_threshold,
            line_11_taxable_income_before_qbi: self.line_11_taxable_income_before_qbi,
            ..Self::default()
        };
    }

    /// Returns `true` when the user has entered anything on the form. Line 11
    /// does not count, because it comes from the estimate.
    pub fn has_entries(&self) -> bool {
        let has_trade = self
            .line_1_trades_or_businesses
            .iter()
            .any(|trade| !trade.is_blank());
        has_trade
            || self.line_3_qbi_loss_carryforward.is_some()
            || self.line_6_reit_ptp_income.is_some()
            || self.line_7_reit_ptp_loss_carryforward.is_some()
            || self.line_12_net_capital_gain.is_some()
    }

    /// Builds the persisted computed lines. `None` until lines 15, 16, and 17
    /// have all been figured.
    pub fn to_computed(&self) -> Option<QbiComputed> {
        Some(QbiComputed {
            qbi_deduction: self.line_15_qbi_deduction?,
            total_qbi_loss_carryforward: self.line_16_total_qbi_loss_carryforward?,
            total_reit_ptp_loss_carryforward: self.line_17_total_reit_ptp_loss_carryforward?,
        })
    }

    /// Builds the persisted record for the estimate with `tax_estimate_id`.
    pub fn to_qbi(
        &self,
        tax_estimate_id: i64,
    ) -> Qbi {
        Qbi {
            tax_estimate_id,
            input: QbiInput::from(self),
            computed: self.to_computed(),
        }
    }

    /// Replaces the entered lines, line 11, and lines 15 through 17 with the
    /// values in `qbi`. Line 1 rows beyond the five on the form are dropped.
    /// The tax year and threshold are kept, and the other computed lines are
    /// left for the next recalculation.
    pub fn populate_from_qbi(
        &mut self,
        qbi: &Qbi,
    ) {
        let input = &qbi.input;

        self.line_1_trades_or_businesses = Default::default();
        for (trade, business) in self
            .line_1_trades_or_businesses
            .iter_mut()
            .zip(&input.businesses)
        {
            *trade = QbiTradeEntry::from(business);
        }
        self.line_3_qbi_loss_carryforward = Some(input.qbi_loss_carryforward);
        self.line_6_reit_ptp_income = Some(input.reit_ptp_income);
        self.line_7_reit_ptp_loss_carryforward = Some(input.reit_ptp_loss_carryforward);
        self.line_11_taxable_income_before_qbi = input.taxable_income_before_qbi;
        self.line_12_net_capital_gain = Some(input.net_capital_gain);

        let computed = qbi.computed.as_ref();
        self.line_15_qbi_deduction = computed.map(|c| c.qbi_deduction);
        self.line_16_total_qbi_loss_carryforward = computed.map(|c| c.total_qbi_loss_carryforward);
        self.line_17_total_reit_ptp_loss_carryforward =
            computed.map(|c| c.total_reit_ptp_loss_carryforward);
    }
}

/// Maps a [`QbiWorksheetResult`] into the computed lines. The entered lines,
/// tax year, and threshold are left empty.
impl From<&QbiWorksheetResult> for QbiWorksheetModel {
    fn from(result: &QbiWorksheetResult) -> Self {
        let mut model = Self::default();
        model.from_worksheet_result(result);
        model
    }
}

impl From<QbiWorksheetResult> for QbiWorksheetModel {
    fn from(result: QbiWorksheetResult) -> Self {
        Self::from(&result)
    }
}

/// Maps a line 1 row into the persisted form. A blank amount is stored as
/// zero.
impl From<&QbiTradeEntry> for QbiBusiness {
    fn from(entry: &QbiTradeEntry) -> Self {
        Self {
            name: entry.name.clone(),
            taxpayer_id: entry.taxpayer_id.clone(),
            qualified_business_income: entry.qbi_or_loss.unwrap_or_default(),
        }
    }
}

impl From<&QbiBusiness> for QbiTradeEntry {
    fn from(business: &QbiBusiness) -> Self {
        Self {
            name: business.name.clone(),
            taxpayer_id: business.taxpayer_id.clone(),
            qbi_or_loss: Some(business.qualified_business_income),
        }
    }
}

/// Maps the entered lines into the persisted form. Blank line 1 rows are
/// left out, and blank amounts are stored as zero.
impl From<&QbiWorksheetModel> for QbiInput {
    fn from(model: &QbiWorksheetModel) -> Self {
        Self {
            businesses: model
                .line_1_trades_or_businesses
                .iter()
                .filter(|trade| !trade.is_blank())
                .map(QbiBusiness::from)
                .collect(),
            qbi_loss_carryforward: model.line_3_qbi_loss_carryforward.unwrap_or_default(),
            reit_ptp_income: model.line_6_reit_ptp_income.unwrap_or_default(),
            reit_ptp_loss_carryforward: model.line_7_reit_ptp_loss_carryforward.unwrap_or_default(),
            taxable_income_before_qbi: model.line_11_taxable_income_before_qbi,
            net_capital_gain: model.line_12_net_capital_gain.unwrap_or_default(),
        }
    }
}

impl fmt::Display for QbiWorksheetModel {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        for (label, trade) in QBI_TRADE_ROW_LABELS
            .iter()
            .zip(&self.line_1_trades_or_businesses)
        {
            writeln!(
                f,
                "Line 1{label} ({}, {}): {}",
                text_or_dash(&trade.name),
                text_or_dash(&mask_taxpayer_id(&trade.taxpayer_id)),
                opt_decimal_display(&trade.qbi_or_loss)
            )?;
        }
        let amounts = [
            ("Line 2 (total QBI or loss)", self.line_2_total_qbi_or_loss),
            (
                "Line 3 (QBI loss carryforward)",
                self.line_3_qbi_loss_carryforward,
            ),
            ("Line 4 (total QBI)", self.line_4_total_qbi),
            ("Line 5 (QBI component)", self.line_5_qbi_component),
            (
                "Line 6 (REIT and PTP income or loss)",
                self.line_6_reit_ptp_income,
            ),
            (
                "Line 7 (REIT and PTP loss carryforward)",
                self.line_7_reit_ptp_loss_carryforward,
            ),
            (
                "Line 8 (total REIT and PTP income)",
                self.line_8_total_reit_ptp_income,
            ),
            (
                "Line 9 (REIT and PTP component)",
                self.line_9_reit_ptp_component,
            ),
            (
                "Line 10 (before income limitation)",
                self.line_10_deduction_before_income_limitation,
            ),
            (
                "Line 11 (taxable income before QBI)",
                self.line_11_taxable_income_before_qbi,
            ),
            ("Line 12 (net capital gain)", self.line_12_net_capital_gain),
            (
                "Line 13 (line 11 − line 12)",
                self.line_13_taxable_income_less_net_capital_gain,
            ),
            (
                "Line 14 (income limitation)",
                self.line_14_income_limitation,
            ),
            ("Line 15 (QBI deduction)", self.line_15_qbi_deduction),
            (
                "Line 16 (QBI loss carryforward)",
                self.line_16_total_qbi_loss_carryforward,
            ),
            (
                "Line 17 (REIT/PTP carryforward)",
                self.line_17_total_reit_ptp_loss_carryforward,
            ),
        ];
        for (label, value) in amounts {
            writeln!(f, "{label}: {}", opt_decimal_display(&value))?;
        }
        write!(f, "Above threshold: {}", self.above_threshold)
    }
}

/// Returns `text`, or "—" when it is empty.
fn text_or_dash(text: &str) -> &str {
    if text.is_empty() { "—" } else { text }
}

/// Masks a taxpayer identification number for display and log output,
/// keeping only the last four digits. Entries with four or fewer digits are
/// masked completely, and a blank entry stays blank.
fn mask_taxpayer_id(taxpayer_id: &str) -> String {
    if taxpayer_id.trim().is_empty() {
        return String::new();
    }
    let digits: Vec<char> = taxpayer_id.chars().filter(char::is_ascii_digit).collect();
    if digits.len() <= 4 {
        return "*****".to_string();
    }
    let last_four: String = digits[digits.len() - 4..].iter().collect();
    format!("*****{last_four}")
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use rust_decimal_macros::dec;

    use super::*;

    #[test]
    fn mask_taxpayer_id_shows_only_the_last_four_digits() {
        assert_eq!(mask_taxpayer_id("123-45-6789"), "*****6789");
        assert_eq!(mask_taxpayer_id("12-3456789"), "*****6789");
        assert_eq!(mask_taxpayer_id("1234"), "*****");
        assert_eq!(mask_taxpayer_id(""), "");
    }

    #[test]
    fn debug_output_masks_the_taxpayer_id() {
        let entry = QbiTradeEntry {
            taxpayer_id: "123-45-6789".to_string(),
            ..QbiTradeEntry::default()
        };

        assert!(!format!("{entry:?}").contains("123-45"));
    }

    #[test]
    fn set_filing_context_sets_threshold_only_for_a_known_year() {
        let mut model = QbiWorksheetModel::default();

        model.set_filing_context(Some(2025), true, Some(dec!(400000)));
        assert_eq!(
            model.worksheet_config().taxable_income_threshold,
            Some(dec!(394600))
        );
        assert_eq!(model.line_11_taxable_income_before_qbi, Some(dec!(400000)));

        model.set_filing_context(None, true, None);
        assert_eq!(model.worksheet_config().taxable_income_threshold, None);
    }

    #[test]
    fn to_worksheet_input_treats_blank_amounts_as_zero() {
        let mut model = QbiWorksheetModel::default();
        model.line_1_trades_or_businesses[0].qbi_or_loss = Some(dec!(1000));
        model.line_1_trades_or_businesses[2].qbi_or_loss = Some(dec!(-250));
        model.line_3_qbi_loss_carryforward = Some(dec!(-100));
        model.line_11_taxable_income_before_qbi = Some(dec!(50000));

        assert_eq!(
            model.to_worksheet_input(),
            QbiWorksheetInput {
                trade_or_business_income: vec![dec!(1000), dec!(-250)],
                qbi_loss_carryforward: dec!(-100),
                taxable_income_before_qbi: Some(dec!(50000)),
                ..QbiWorksheetInput::default()
            }
        );
    }

    #[test]
    fn from_worksheet_result_maps_every_computed_line() {
        let result = QbiWorksheetResult {
            total_qbi_or_loss: dec!(10000),
            total_qbi: dec!(10000),
            qbi_component: dec!(2000),
            total_reit_ptp_income: dec!(4000),
            reit_ptp_component: dec!(800),
            deduction_before_income_limitation: dec!(2800),
            taxable_income_before_qbi: Some(dec!(100000)),
            taxable_income_less_net_capital_gain: Some(dec!(100000)),
            income_limitation: Some(dec!(20000)),
            qbi_deduction: dec!(2800),
            total_qbi_loss_carryforward: Decimal::ZERO,
            total_reit_ptp_loss_carryforward: dec!(-50),
            above_threshold: true,
        };

        assert_eq!(
            QbiWorksheetModel::from(&result),
            QbiWorksheetModel {
                line_2_total_qbi_or_loss: Some(dec!(10000)),
                line_4_total_qbi: Some(dec!(10000)),
                line_5_qbi_component: Some(dec!(2000)),
                line_8_total_reit_ptp_income: Some(dec!(4000)),
                line_9_reit_ptp_component: Some(dec!(800)),
                line_10_deduction_before_income_limitation: Some(dec!(2800)),
                line_11_taxable_income_before_qbi: Some(dec!(100000)),
                line_13_taxable_income_less_net_capital_gain: Some(dec!(100000)),
                line_14_income_limitation: Some(dec!(20000)),
                line_15_qbi_deduction: Some(dec!(2800)),
                line_16_total_qbi_loss_carryforward: Some(Decimal::ZERO),
                line_17_total_reit_ptp_loss_carryforward: Some(dec!(-50)),
                above_threshold: true,
                ..QbiWorksheetModel::default()
            }
        );
    }

    #[test]
    fn clear_entries_keeps_values_from_the_estimate() {
        let mut model = QbiWorksheetModel::default();
        model.set_filing_context(Some(2025), false, Some(dec!(80000)));
        model.line_1_trades_or_businesses[0].name = "Consulting".to_string();
        model.line_3_qbi_loss_carryforward = Some(dec!(-100));
        model.line_15_qbi_deduction = Some(dec!(500));
        model.above_threshold = true;

        model.clear_entries();

        let mut expected = QbiWorksheetModel::default();
        expected.set_filing_context(Some(2025), false, Some(dec!(80000)));
        assert_eq!(model, expected);
    }

    /// Rows i and iii filled in, row iii being an aggregation with no
    /// taxpayer identification number.
    fn entered_model() -> QbiWorksheetModel {
        let mut model = QbiWorksheetModel::default();
        model.line_1_trades_or_businesses[0] = QbiTradeEntry {
            name: "Consulting".to_string(),
            taxpayer_id: "12-3456789".to_string(),
            qbi_or_loss: Some(dec!(70000.50)),
        };
        model.line_1_trades_or_businesses[2] = QbiTradeEntry {
            name: "Aggregation 1".to_string(),
            taxpayer_id: String::new(),
            qbi_or_loss: Some(dec!(-1200)),
        };
        model.line_3_qbi_loss_carryforward = Some(dec!(-3000));
        model.line_11_taxable_income_before_qbi = Some(dec!(80000.25));
        model.line_12_net_capital_gain = Some(dec!(550.75));
        model
    }

    #[test]
    fn qbi_input_skips_blank_rows_and_stores_blank_amounts_as_zero() {
        assert_eq!(
            QbiInput::from(&entered_model()),
            QbiInput {
                businesses: vec![
                    QbiBusiness {
                        name: "Consulting".to_string(),
                        taxpayer_id: "12-3456789".to_string(),
                        qualified_business_income: dec!(70000.50),
                    },
                    QbiBusiness {
                        name: "Aggregation 1".to_string(),
                        taxpayer_id: String::new(),
                        qualified_business_income: dec!(-1200),
                    },
                ],
                qbi_loss_carryforward: dec!(-3000),
                reit_ptp_income: Decimal::ZERO,
                reit_ptp_loss_carryforward: Decimal::ZERO,
                taxable_income_before_qbi: Some(dec!(80000.25)),
                net_capital_gain: dec!(550.75),
            }
        );
    }

    #[test]
    fn has_entries_ignores_values_from_the_estimate() {
        let mut model = QbiWorksheetModel::default();
        model.set_filing_context(Some(2025), false, Some(dec!(80000)));
        assert!(!model.has_entries());

        model.line_6_reit_ptp_income = Some(Decimal::ZERO);
        assert!(model.has_entries());
    }

    #[test]
    fn to_computed_needs_every_stored_line() {
        let mut model = QbiWorksheetModel::default();
        assert_eq!(model.to_computed(), None);

        model.line_15_qbi_deduction = Some(dec!(500));
        model.line_16_total_qbi_loss_carryforward = Some(Decimal::ZERO);
        assert_eq!(model.to_computed(), None);

        model.line_17_total_reit_ptp_loss_carryforward = Some(dec!(-50));
        assert_eq!(
            model.to_computed(),
            Some(QbiComputed {
                qbi_deduction: dec!(500),
                total_qbi_loss_carryforward: Decimal::ZERO,
                total_reit_ptp_loss_carryforward: dec!(-50),
            })
        );
    }

    #[test]
    fn populate_from_qbi_round_trips_the_persisted_record() {
        let mut model = entered_model();
        model.line_15_qbi_deduction = Some(dec!(13000));
        model.line_16_total_qbi_loss_carryforward = Some(Decimal::ZERO);
        model.line_17_total_reit_ptp_loss_carryforward = Some(Decimal::ZERO);
        let qbi = model.to_qbi(42);
        assert_eq!(qbi.tax_estimate_id, 42);

        let mut loaded = QbiWorksheetModel::default();
        loaded.set_filing_context(Some(2025), false, None);
        loaded.line_1_trades_or_businesses[4].name = "Stale".to_string();
        loaded.populate_from_qbi(&qbi);

        assert_eq!(QbiInput::from(&loaded), qbi.input);
        assert_eq!(loaded.to_computed(), qbi.computed);
        assert_eq!(loaded.tax_year, Some(2025));
        // Stored rows fill the form from the top; the rest are cleared.
        assert_eq!(loaded.line_1_trades_or_businesses[1].name, "Aggregation 1");
        assert!(loaded.line_1_trades_or_businesses[4].is_blank());
    }

    #[test]
    fn display_uses_em_dash_for_missing_values() {
        const EXPECTED: &str = "Line 1i (—, —): —
Line 1ii (—, —): —
Line 1iii (—, —): —
Line 1iv (—, —): —
Line 1v (—, —): —
Line 2 (total QBI or loss): —
Line 3 (QBI loss carryforward): —
Line 4 (total QBI): —
Line 5 (QBI component): —
Line 6 (REIT and PTP income or loss): —
Line 7 (REIT and PTP loss carryforward): —
Line 8 (total REIT and PTP income): —
Line 9 (REIT and PTP component): —
Line 10 (before income limitation): —
Line 11 (taxable income before QBI): —
Line 12 (net capital gain): —
Line 13 (line 11 − line 12): —
Line 14 (income limitation): —
Line 15 (QBI deduction): —
Line 16 (QBI loss carryforward): —
Line 17 (REIT/PTP carryforward): —
Above threshold: false";

        assert_eq!(QbiWorksheetModel::default().to_string(), EXPECTED);
    }
}
