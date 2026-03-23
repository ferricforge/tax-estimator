use std::fmt;

use rust_decimal::Decimal;
use tax_core::calculations::SeWorksheetResult;

use crate::utils::opt_decimal_display;

/// Form 1040-ES “Self-Employment Tax and Deduction Worksheet” (lines 1a–11).
///
/// Field names follow IRS line numbers; see the project `docs/SeWorksheet.md` (Form 1040-ES).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SeWorksheetModel {
    /// Line 1a: Expected income and profits subject to self-employment tax.
    pub line_1a_expected_se_income: Option<Decimal>,
    /// Line 1b: Expected Conservation Reserve Program payments (when applicable).
    pub line_1b_expected_crp_payments: Option<Decimal>,
    /// Line 2: Subtract line 1b from line 1a.
    pub line_2_subtract_1b_from_1a: Option<Decimal>,
    /// Line 3: Multiply line 2 by 92.35% (0.9235).
    pub line_3_net_earnings: Option<Decimal>,
    /// Line 4: Multiply line 3 by 2.9% (0.029) — Medicare component.
    pub line_4_medicare_tax: Option<Decimal>,
    /// Line 5: Social security tax maximum income (e.g. annual SS wage base).
    pub line_5_ss_maximum_income: Option<Decimal>,
    /// Line 6: Expected wages (subject to social security tax or 6.2% tier 1 RRTA).
    pub line_6_expected_wages: Option<Decimal>,
    /// Line 7: Subtract line 6 from line 5.
    pub line_7_remaining_ss_base: Option<Decimal>,
    /// Line 8: Enter the smaller of line 3 or line 7.
    pub line_8_ss_taxable_earnings: Option<Decimal>,
    /// Line 9: Multiply line 8 by 12.4% (0.124) — social security component.
    pub line_9_social_security_tax: Option<Decimal>,
    /// Line 10: Add lines 4 and 9.
    pub line_10_total_se_tax: Option<Decimal>,
    /// Line 11: Multiply line 10 by 50% (0.50) — deductible SE tax.
    pub line_11_deductible_se_tax: Option<Decimal>,

    pub tax_year: Option<i32>,
}

impl SeWorksheetModel {
    // TODO: Account for Line 5, base SSA salary
    pub fn from_worksheet_result(
        &mut self,
        result: &SeWorksheetResult,
    ) {
        self.line_2_subtract_1b_from_1a = Some(result.combined_se_income);
        self.line_3_net_earnings = Some(result.net_earnings);
        self.line_4_medicare_tax = Some(result.medicare_tax);
        self.line_8_ss_taxable_earnings = Some(result.ss_taxable_earnings);
        self.line_9_social_security_tax = Some(result.social_security_tax);
        self.line_10_total_se_tax = Some(result.self_employment_tax);
        self.line_11_deductible_se_tax = Some(result.se_tax_deduction);
    }
}

/// Maps [`tax_core::calculations::SeWorksheetResult`] into IRS-aligned lines.
///
/// Fills lines **3**, **4**, **8**, **9**, **10**, and **11** from the calculator. Lines **1a**, **1b**,
/// **2**, **5**, **6**, and **7** are [`None`] (user inputs or config not present on the result type).
impl From<&SeWorksheetResult> for SeWorksheetModel {
    fn from(result: &SeWorksheetResult) -> Self {
        Self {
            line_1a_expected_se_income: None,
            line_1b_expected_crp_payments: None,
            line_2_subtract_1b_from_1a: None,
            line_3_net_earnings: Some(result.net_earnings),
            line_4_medicare_tax: Some(result.medicare_tax),
            line_5_ss_maximum_income: None,
            line_6_expected_wages: None,
            line_7_remaining_ss_base: None,
            line_8_ss_taxable_earnings: Some(result.ss_taxable_earnings),
            line_9_social_security_tax: Some(result.social_security_tax),
            line_10_total_se_tax: Some(result.self_employment_tax),
            line_11_deductible_se_tax: Some(result.se_tax_deduction),
            ..Default::default()
        }
    }
}

impl From<SeWorksheetResult> for SeWorksheetModel {
    fn from(result: SeWorksheetResult) -> Self {
        Self::from(&result)
    }
}

impl fmt::Display for SeWorksheetModel {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        writeln!(
            f,
            "Line 1a (expected SE income): {}",
            opt_decimal_display(&self.line_1a_expected_se_income)
        )?;
        writeln!(
            f,
            "Line 1b (expected CRP):         {}",
            opt_decimal_display(&self.line_1b_expected_crp_payments)
        )?;
        writeln!(
            f,
            "Line 2 (line 1a − line 1b):    {}",
            opt_decimal_display(&self.line_2_subtract_1b_from_1a)
        )?;
        writeln!(
            f,
            "Line 3 (line 2 × 92.35%):     {}",
            opt_decimal_display(&self.line_3_net_earnings)
        )?;
        writeln!(
            f,
            "Line 4 (line 3 × 2.9%):       {}",
            opt_decimal_display(&self.line_4_medicare_tax)
        )?;
        writeln!(
            f,
            "Line 5 (SS maximum income):   {}",
            opt_decimal_display(&self.line_5_ss_maximum_income)
        )?;
        writeln!(
            f,
            "Line 6 (expected wages):      {}",
            opt_decimal_display(&self.line_6_expected_wages)
        )?;
        writeln!(
            f,
            "Line 7 (line 5 − line 6):     {}",
            opt_decimal_display(&self.line_7_remaining_ss_base)
        )?;
        writeln!(
            f,
            "Line 8 (smaller of 3 or 7):   {}",
            opt_decimal_display(&self.line_8_ss_taxable_earnings)
        )?;
        writeln!(
            f,
            "Line 9 (line 8 × 12.4%):      {}",
            opt_decimal_display(&self.line_9_social_security_tax)
        )?;
        writeln!(
            f,
            "Line 10 (line 4 + line 9):    {}",
            opt_decimal_display(&self.line_10_total_se_tax)
        )?;
        write!(
            f,
            "Line 11 (line 10 × 50%):      {}",
            opt_decimal_display(&self.line_11_deductible_se_tax)
        )
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use rust_decimal_macros::dec;
    use tax_core::calculations::SeWorksheetResult;

    use super::*;

    /// Golden output for [`SeWorksheetModel::fmt`] — all lines but the last end with `\n`.
    const DISPLAY_FULL: &str = r"Line 1a (expected SE income): 10000
Line 1b (expected CRP):         2500
Line 2 (line 1a − line 1b):    7500
Line 3 (line 2 × 92.35%):     6926.25
Line 4 (line 3 × 2.9%):       29
Line 5 (SS maximum income):   176100
Line 6 (expected wages):      50000
Line 7 (line 5 − line 6):     126100
Line 8 (smaller of 3 or 7):   6926.25
Line 9 (line 8 × 12.4%):      858.86
Line 10 (line 4 + line 9):    887.86
Line 11 (line 10 × 50%):      443.93";

    #[test]
    fn display_matches_expected_lines() {
        let model = SeWorksheetModel {
            line_1a_expected_se_income: Some(dec!(10000)),
            line_1b_expected_crp_payments: Some(dec!(2500)),
            line_2_subtract_1b_from_1a: Some(dec!(7500)),
            line_3_net_earnings: Some(dec!(6926.25)),
            line_4_medicare_tax: Some(dec!(29)),
            line_5_ss_maximum_income: Some(dec!(176100)),
            line_6_expected_wages: Some(dec!(50000)),
            line_7_remaining_ss_base: Some(dec!(126100)),
            line_8_ss_taxable_earnings: Some(dec!(6926.25)),
            line_9_social_security_tax: Some(dec!(858.86)),
            line_10_total_se_tax: Some(dec!(887.86)),
            line_11_deductible_se_tax: Some(dec!(443.93)),
            ..Default::default()
        };
        let actual = model.to_string();
        assert_eq!(actual, DISPLAY_FULL);
    }

    #[test]
    fn display_uses_em_dash_for_missing_values() {
        let model = SeWorksheetModel::default();
        const EXPECTED: &str = r"Line 1a (expected SE income): —
Line 1b (expected CRP):         —
Line 2 (line 1a − line 1b):    —
Line 3 (line 2 × 92.35%):     —
Line 4 (line 3 × 2.9%):       —
Line 5 (SS maximum income):   —
Line 6 (expected wages):      —
Line 7 (line 5 − line 6):     —
Line 8 (smaller of 3 or 7):   —
Line 9 (line 8 × 12.4%):      —
Line 10 (line 4 + line 9):    —
Line 11 (line 10 × 50%):      —";
        assert_eq!(model.to_string(), EXPECTED);
    }

    #[test]
    fn default_sets_every_line_to_none() {
        let actual = SeWorksheetModel::default();
        let expected = SeWorksheetModel {
            line_1a_expected_se_income: None,
            line_1b_expected_crp_payments: None,
            line_2_subtract_1b_from_1a: None,
            line_3_net_earnings: None,
            line_4_medicare_tax: None,
            line_5_ss_maximum_income: None,
            line_6_expected_wages: None,
            line_7_remaining_ss_base: None,
            line_8_ss_taxable_earnings: None,
            line_9_social_security_tax: None,
            line_10_total_se_tax: None,
            line_11_deductible_se_tax: None,
            ..Default::default()
        };
        assert_eq!(actual, expected);
    }

    #[test]
    fn from_se_worksheet_result_maps_core_fields() {
        let result = SeWorksheetResult {
            combined_se_income: dec!(50_000),
            net_earnings: dec!(46_175),
            medicare_tax: dec!(1339.08),
            ss_taxable_earnings: dec!(46_175),
            social_security_tax: dec!(5725.70),
            self_employment_tax: dec!(7064.78),
            se_tax_deduction: dec!(3532.39),
            below_threshold: false,
        };
        let expected = SeWorksheetModel {
            line_1a_expected_se_income: None,
            line_1b_expected_crp_payments: None,
            line_2_subtract_1b_from_1a: None,
            line_3_net_earnings: Some(dec!(46_175)),
            line_4_medicare_tax: Some(dec!(1339.08)),
            line_5_ss_maximum_income: None,
            line_6_expected_wages: None,
            line_7_remaining_ss_base: None,
            line_8_ss_taxable_earnings: Some(dec!(46_175)),
            line_9_social_security_tax: Some(dec!(5725.70)),
            line_10_total_se_tax: Some(dec!(7064.78)),
            line_11_deductible_se_tax: Some(dec!(3532.39)),
            ..Default::default()
        };
        assert_eq!(SeWorksheetModel::from(&result), expected);
        assert_eq!(SeWorksheetModel::from(result.clone()), expected);
    }

    #[test]
    fn from_se_worksheet_result_below_threshold_uses_zero_amounts() {
        let result = SeWorksheetResult {
            combined_se_income: dec!(400),
            net_earnings: Decimal::ZERO,
            medicare_tax: Decimal::ZERO,
            ss_taxable_earnings: Decimal::ZERO,
            social_security_tax: Decimal::ZERO,
            self_employment_tax: Decimal::ZERO,
            se_tax_deduction: Decimal::ZERO,
            below_threshold: true,
        };
        let expected = SeWorksheetModel {
            line_1a_expected_se_income: None,
            line_1b_expected_crp_payments: None,
            line_2_subtract_1b_from_1a: None,
            line_3_net_earnings: Some(Decimal::ZERO),
            line_4_medicare_tax: Some(Decimal::ZERO),
            line_5_ss_maximum_income: None,
            line_6_expected_wages: None,
            line_7_remaining_ss_base: None,
            line_8_ss_taxable_earnings: Some(Decimal::ZERO),
            line_9_social_security_tax: Some(Decimal::ZERO),
            line_10_total_se_tax: Some(Decimal::ZERO),
            line_11_deductible_se_tax: Some(Decimal::ZERO),
            ..Default::default()
        };
        assert_eq!(SeWorksheetModel::from(&result), expected);
    }
}
