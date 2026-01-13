//! Estimated Tax Worksheet calculations for IRS Form 1040-ES.
//!
//! This module implements the 2025 Estimated Tax Worksheet from Form 1040-ES,
//! which calculates the total estimated tax liability and required payments.
//!
//! # Worksheet Structure
//!
//! The estimated tax worksheet consists of the following lines:
//!
//! | Line | Description |
//! |------|-------------|
//! | 1    | Adjusted gross income (AGI) you expect in 2025 |
//! | 2a   | Deductions (itemized or standard deduction) |
//! | 2b   | Qualified business income (QBI) deduction |
//! | 2c   | Total deductions (Line 2a + Line 2b) |
//! | 3    | Taxable income (Line 1 - Line 2c) |
//! | 4    | Tax (using tax rate schedules) |
//! | 5    | Alternative minimum tax (AMT) |
//! | 6    | Total tax before credits (Line 4 + Line 5) |
//! | 7    | Credits (excluding withholding) |
//! | 8    | Tax after credits (Line 6 - Line 7, minimum 0) |
//! | 9    | Self-employment tax |
//! | 10   | Other taxes |
//! | 11a  | Total tax (Line 8 + Line 9 + Line 10) |
//! | 11b  | Refundable credits |
//! | 11c  | Total 2025 estimated tax (Line 11a - Line 11b, minimum 0) |
//! | 12a  | Line 11c × 90% (or 66⅔% for farmers/fishers) |
//! | 12b  | Prior year's tax (100% or 110% for high earners) |
//! | 12c  | Required annual payment (smaller of 12a or 12b) |
//! | 13   | Income tax withheld during 2025 |
//! | 14a  | Line 12c - Line 13 (if ≤0, no estimated payments required) |
//! | 14b  | Line 11c - Line 13 (if <$1000, no estimated payments required) |
//!
//! # Example
//!
//! ```
//! use rust_decimal_macros::dec;
//! use tax_core::calculations::{EstimatedTaxWorksheet, EstimatedTaxWorksheetInput};
//! use tax_core::TaxBracket;
//!
//! let tax_brackets = vec![
//!     TaxBracket {
//!         tax_year: 2025,
//!         filing_status_id: 1,
//!         min_income: dec!(0),
//!         max_income: Some(dec!(11925)),
//!         tax_rate: dec!(0.10),
//!         base_tax: dec!(0),
//!     },
//!     TaxBracket {
//!         tax_year: 2025,
//!         filing_status_id: 1,
//!         min_income: dec!(11925),
//!         max_income: Some(dec!(48475)),
//!         tax_rate: dec!(0.12),
//!         base_tax: dec!(1192.50),
//!     },
//!     TaxBracket {
//!         tax_year: 2025,
//!         filing_status_id: 1,
//!         min_income: dec!(48475),
//!         max_income: Some(dec!(103350)),
//!         tax_rate: dec!(0.22),
//!         base_tax: dec!(5578.50),
//!     },
//! ];
//!
//! let input = EstimatedTaxWorksheetInput {
//!     adjusted_gross_income: dec!(100000.00),
//!     itemized_deduction: dec!(0.00),
//!     standard_deduction: dec!(15000.00),
//!     qbi_deduction: dec!(0.00),
//!     alternative_minimum_tax: dec!(0.00),
//!     credits: dec!(0.00),
//!     self_employment_tax: dec!(0.00),
//!     other_taxes: dec!(0.00),
//!     refundable_credits: dec!(0.00),
//!     prior_year_tax: dec!(12000.00),
//!     withholding: dec!(0.00),
//!     is_farmer_or_fisher: false,
//!     required_payment_threshold: dec!(1000.00),
//! };
//!
//! let worksheet = EstimatedTaxWorksheet::new(&tax_brackets);
//! let result = worksheet.calculate(&input).unwrap();
//!
//! assert_eq!(result.total_estimated_tax, dec!(13614.00));
//! assert_eq!(result.required_annual_payment, dec!(12000.00));
//! assert!(result.estimated_payments_required);
//! ```

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::TaxBracket;
use crate::calculations::common::{max, round_half_up};

/// Errors that can occur during estimated tax worksheet calculations.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum EstimatedTaxWorksheetError {
    /// No tax brackets were provided for the calculation.
    #[error("no tax brackets provided")]
    NoTaxBrackets,

    /// No tax bracket found for the given taxable income.
    #[error("no tax bracket found for taxable income {0}")]
    NoMatchingBracket(Decimal),
}

/// Input values for the Estimated Tax Worksheet.
///
/// These values are typically provided by the user and correspond to the
/// input fields on Form 1040-ES.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EstimatedTaxWorksheetInput {
    /// Adjusted gross income expected for the tax year.
    pub adjusted_gross_income: Decimal,

    /// Itemized deduction amount.
    /// If greater than zero, this will be used instead of standard deduction.
    pub itemized_deduction: Decimal,

    /// Standard deduction for the filing status.
    /// Used if itemized deduction is zero or less.
    pub standard_deduction: Decimal,

    /// Qualified business income (QBI) deduction.
    pub qbi_deduction: Decimal,

    /// Alternative minimum tax from Form 6251.
    pub alternative_minimum_tax: Decimal,

    /// Credits (excluding income tax withholding).
    pub credits: Decimal,

    /// Self-employment tax from SE worksheet.
    pub self_employment_tax: Decimal,

    /// Other taxes (household employment, NIIT, etc.).
    pub other_taxes: Decimal,

    /// Refundable credits (earned income credit, additional child tax credit, etc.).
    pub refundable_credits: Decimal,

    /// Prior year's tax liability.
    /// This should already be adjusted (100% or 110%) based on prior year AGI.
    pub prior_year_tax: Decimal,

    /// Income tax withheld and estimated to be withheld during 2025.
    pub withholding: Decimal,

    /// Whether the taxpayer qualifies as a farmer or fisher.
    /// If true, uses 66⅔% instead of 90% for current year factor.
    pub is_farmer_or_fisher: bool,

    /// The threshold for determining if estimated payments are required.
    /// This is typically $1,000 from TaxYearConfig.required_payment_threshold.
    pub required_payment_threshold: Decimal,
}

/// Result of the Estimated Tax Worksheet calculations.
///
/// Contains the key output values needed for estimated tax payment planning.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EstimatedTaxWorksheetResult {
    /// Taxable income after deductions.
    pub taxable_income: Decimal,

    /// Tax calculated from the tax rate schedules.
    pub calculated_tax: Decimal,

    /// Total estimated tax for the year (line 11c).
    pub total_estimated_tax: Decimal,

    /// Required annual payment to avoid penalty (line 12c).
    /// This is the smaller of 90% of current year tax or prior year tax.
    pub required_annual_payment: Decimal,

    /// Amount that must be paid via estimated payments (line 14a).
    /// This is the required payment minus withholding.
    pub underpayment: Decimal,

    /// Indicates whether itemized deduction was used instead of standard.
    pub used_itemized_deduction: bool,

    /// Indicates whether estimated tax payments are required.
    /// False if withholding covers the requirement or if under the threshold.
    pub estimated_payments_required: bool,
}

/// Calculator for the Estimated Tax Worksheet.
///
/// This struct encapsulates the tax brackets and provides methods to calculate
/// estimated tax liability and required payments.
#[derive(Debug, Clone)]
pub struct EstimatedTaxWorksheet<'a> {
    tax_brackets: &'a [TaxBracket],
}

impl<'a> EstimatedTaxWorksheet<'a> {
    /// Creates a new Estimated Tax Worksheet calculator with the given tax brackets.
    ///
    /// Tax brackets should be sorted by `min_income` in ascending order and
    /// must cover all income ranges (the last bracket should have `max_income`
    /// as `None`).
    pub fn new(tax_brackets: &'a [TaxBracket]) -> Self {
        Self { tax_brackets }
    }

    /// Calculates the complete Estimated Tax Worksheet.
    ///
    /// This is the main entry point for estimated tax calculations. It performs
    /// all line calculations and returns the key results needed for tax planning.
    ///
    /// # Errors
    ///
    /// Returns [`EstimatedTaxWorksheetError`] if:
    /// - No tax brackets were provided
    /// - No matching bracket found for the taxable income
    pub fn calculate(
        &self,
        input: &EstimatedTaxWorksheetInput,
    ) -> Result<EstimatedTaxWorksheetResult, EstimatedTaxWorksheetError> {
        if self.tax_brackets.is_empty() {
            return Err(EstimatedTaxWorksheetError::NoTaxBrackets);
        }

        // Determine deduction (itemized if > 0, else standard)
        let (deduction, used_itemized) =
            self.determine_deduction(input.itemized_deduction, input.standard_deduction);

        // Calculate total deductions
        let total_deductions = self.total_deductions(deduction, input.qbi_deduction);

        // Calculate taxable income
        let taxable_income = self.taxable_income(input.adjusted_gross_income, total_deductions);

        // Calculate tax from schedules
        let calculated_tax = self.calculate_tax(taxable_income)?;

        // Calculate total tax before credits (tax + AMT)
        let total_tax_before_credits =
            self.total_tax_before_credits(calculated_tax, input.alternative_minimum_tax);

        // Apply credits
        let tax_after_credits = self.tax_after_credits(total_tax_before_credits, input.credits);

        // Add SE tax and other taxes
        let total_tax = self.total_tax(
            tax_after_credits,
            input.self_employment_tax,
            input.other_taxes,
        );

        // Subtract refundable credits to get total estimated tax
        let total_estimated_tax = self.total_estimated_tax(total_tax, input.refundable_credits);

        // Calculate required annual payment
        let current_year_factor =
            self.current_year_factor(total_estimated_tax, input.is_farmer_or_fisher);
        let required_annual_payment =
            self.required_annual_payment(current_year_factor, input.prior_year_tax);

        // Calculate underpayment (balance due via estimated payments)
        let underpayment = self.underpayment(required_annual_payment, input.withholding);

        // Determine if estimated payments are required
        let threshold_amount = self.threshold_amount(total_estimated_tax, input.withholding);
        let estimated_payments_required = self.are_estimated_payments_required(
            underpayment,
            threshold_amount,
            input.required_payment_threshold,
        );

        Ok(EstimatedTaxWorksheetResult {
            taxable_income,
            calculated_tax,
            total_estimated_tax,
            required_annual_payment,
            underpayment,
            used_itemized_deduction: used_itemized,
            estimated_payments_required,
        })
    }

    /// Determines which deduction to use.
    ///
    /// If itemized deduction is greater than zero, uses itemized deduction.
    /// Otherwise, uses standard deduction.
    fn determine_deduction(
        &self,
        itemized: Decimal,
        standard: Decimal,
    ) -> (Decimal, bool) {
        if itemized > Decimal::ZERO {
            (round_half_up(itemized), true)
        } else {
            (round_half_up(standard), false)
        }
    }

    /// Calculates total deductions.
    fn total_deductions(
        &self,
        deduction: Decimal,
        qbi_deduction: Decimal,
    ) -> Decimal {
        round_half_up(deduction + qbi_deduction)
    }

    /// Calculates taxable income.
    fn taxable_income(
        &self,
        agi: Decimal,
        total_deductions: Decimal,
    ) -> Decimal {
        max(round_half_up(agi - total_deductions), Decimal::ZERO)
    }

    /// Calculates tax using the tax rate schedules.
    fn calculate_tax(
        &self,
        taxable_income: Decimal,
    ) -> Result<Decimal, EstimatedTaxWorksheetError> {
        if taxable_income <= Decimal::ZERO {
            return Ok(Decimal::ZERO);
        }

        let bracket = self
            .tax_brackets
            .iter()
            .find(|b| {
                taxable_income > b.min_income
                    && (b.max_income.is_none()
                        || taxable_income <= b.max_income.unwrap_or(Decimal::MAX))
            })
            .ok_or(EstimatedTaxWorksheetError::NoMatchingBracket(
                taxable_income,
            ))?;

        let marginal_income = taxable_income - bracket.min_income;
        let tax = bracket.base_tax + (marginal_income * bracket.tax_rate);

        Ok(round_half_up(tax))
    }

    /// Calculates total tax before credits.
    fn total_tax_before_credits(
        &self,
        tax: Decimal,
        amt: Decimal,
    ) -> Decimal {
        round_half_up(tax + amt)
    }

    /// Calculates tax after credits.
    fn tax_after_credits(
        &self,
        total_tax_before_credits: Decimal,
        credits: Decimal,
    ) -> Decimal {
        max(
            round_half_up(total_tax_before_credits - credits),
            Decimal::ZERO,
        )
    }

    /// Calculates total tax (before refundable credits).
    fn total_tax(
        &self,
        tax_after_credits: Decimal,
        se_tax: Decimal,
        other_taxes: Decimal,
    ) -> Decimal {
        round_half_up(tax_after_credits + se_tax + other_taxes)
    }

    /// Calculates total estimated tax.
    fn total_estimated_tax(
        &self,
        total_tax: Decimal,
        refundable_credits: Decimal,
    ) -> Decimal {
        max(round_half_up(total_tax - refundable_credits), Decimal::ZERO)
    }

    /// Calculates current year factor (90% or 66⅔% for farmers/fishers).
    fn current_year_factor(
        &self,
        total_estimated_tax: Decimal,
        is_farmer_or_fisher: bool,
    ) -> Decimal {
        let factor = if is_farmer_or_fisher {
            Decimal::TWO / Decimal::from(3)
        } else {
            Decimal::new(90, 2)
        };
        round_half_up(total_estimated_tax * factor)
    }

    /// Calculates required annual payment (smaller of current year factor or prior year tax).
    fn required_annual_payment(
        &self,
        current_year_factor: Decimal,
        prior_year_tax: Decimal,
    ) -> Decimal {
        current_year_factor.min(prior_year_tax)
    }

    /// Calculates underpayment (required payment minus withholding).
    fn underpayment(
        &self,
        required_annual_payment: Decimal,
        withholding: Decimal,
    ) -> Decimal {
        max(
            round_half_up(required_annual_payment - withholding),
            Decimal::ZERO,
        )
    }

    /// Calculates threshold amount (total tax minus withholding).
    fn threshold_amount(
        &self,
        total_estimated_tax: Decimal,
        withholding: Decimal,
    ) -> Decimal {
        max(
            round_half_up(total_estimated_tax - withholding),
            Decimal::ZERO,
        )
    }

    /// Determines if estimated tax payments are required.
    fn are_estimated_payments_required(
        &self,
        underpayment: Decimal,
        threshold_amount: Decimal,
        threshold: Decimal,
    ) -> bool {
        underpayment > Decimal::ZERO && threshold_amount >= threshold
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use rust_decimal_macros::dec;

    use super::*;

    fn test_brackets_single() -> Vec<TaxBracket> {
        vec![
            TaxBracket {
                tax_year: 2025,
                filing_status_id: 1,
                min_income: dec!(0),
                max_income: Some(dec!(11925)),
                tax_rate: dec!(0.10),
                base_tax: dec!(0),
            },
            TaxBracket {
                tax_year: 2025,
                filing_status_id: 1,
                min_income: dec!(11925),
                max_income: Some(dec!(48475)),
                tax_rate: dec!(0.12),
                base_tax: dec!(1192.50),
            },
            TaxBracket {
                tax_year: 2025,
                filing_status_id: 1,
                min_income: dec!(48475),
                max_income: Some(dec!(103350)),
                tax_rate: dec!(0.22),
                base_tax: dec!(5578.50),
            },
            TaxBracket {
                tax_year: 2025,
                filing_status_id: 1,
                min_income: dec!(103350),
                max_income: Some(dec!(197300)),
                tax_rate: dec!(0.24),
                base_tax: dec!(17651),
            },
            TaxBracket {
                tax_year: 2025,
                filing_status_id: 1,
                min_income: dec!(197300),
                max_income: Some(dec!(250525)),
                tax_rate: dec!(0.32),
                base_tax: dec!(40199),
            },
            TaxBracket {
                tax_year: 2025,
                filing_status_id: 1,
                min_income: dec!(250525),
                max_income: Some(dec!(626350)),
                tax_rate: dec!(0.35),
                base_tax: dec!(57231),
            },
            TaxBracket {
                tax_year: 2025,
                filing_status_id: 1,
                min_income: dec!(626350),
                max_income: None,
                tax_rate: dec!(0.37),
                base_tax: dec!(188769.75),
            },
        ]
    }

    fn test_input() -> EstimatedTaxWorksheetInput {
        EstimatedTaxWorksheetInput {
            adjusted_gross_income: dec!(100000.00),
            itemized_deduction: dec!(0.00),
            standard_deduction: dec!(15000.00),
            qbi_deduction: dec!(0.00),
            alternative_minimum_tax: dec!(0.00),
            credits: dec!(0.00),
            self_employment_tax: dec!(0.00),
            other_taxes: dec!(0.00),
            refundable_credits: dec!(0.00),
            prior_year_tax: dec!(12000.00),
            withholding: dec!(0.00),
            is_farmer_or_fisher: false,
            required_payment_threshold: dec!(1000.00),
        }
    }

    // =========================================================================
    // determine_deduction tests
    // =========================================================================

    #[test]
    fn determine_deduction_uses_itemized_when_positive() {
        let brackets = test_brackets_single();
        let worksheet = EstimatedTaxWorksheet::new(&brackets);

        let (deduction, used_itemized) =
            worksheet.determine_deduction(dec!(20000.00), dec!(15000.00));

        assert_eq!(deduction, dec!(20000.00));
        assert!(used_itemized);
    }

    #[test]
    fn determine_deduction_uses_standard_when_itemized_zero() {
        let brackets = test_brackets_single();
        let worksheet = EstimatedTaxWorksheet::new(&brackets);

        let (deduction, used_itemized) = worksheet.determine_deduction(dec!(0.00), dec!(15000.00));

        assert_eq!(deduction, dec!(15000.00));
        assert!(!used_itemized);
    }

    #[test]
    fn determine_deduction_uses_standard_when_itemized_negative() {
        let brackets = test_brackets_single();
        let worksheet = EstimatedTaxWorksheet::new(&brackets);

        let (deduction, used_itemized) =
            worksheet.determine_deduction(dec!(-100.00), dec!(15000.00));

        assert_eq!(deduction, dec!(15000.00));
        assert!(!used_itemized);
    }

    // =========================================================================
    // total_deductions tests
    // =========================================================================

    #[test]
    fn total_deductions_adds_deduction_and_qbi() {
        let brackets = test_brackets_single();
        let worksheet = EstimatedTaxWorksheet::new(&brackets);

        let result = worksheet.total_deductions(dec!(15000.00), dec!(5000.00));

        assert_eq!(result, dec!(20000.00));
    }

    // =========================================================================
    // taxable_income tests
    // =========================================================================

    #[test]
    fn taxable_income_subtracts_deductions_from_agi() {
        let brackets = test_brackets_single();
        let worksheet = EstimatedTaxWorksheet::new(&brackets);

        let result = worksheet.taxable_income(dec!(100000.00), dec!(15000.00));

        assert_eq!(result, dec!(85000.00));
    }

    #[test]
    fn taxable_income_returns_zero_when_deductions_exceed_agi() {
        let brackets = test_brackets_single();
        let worksheet = EstimatedTaxWorksheet::new(&brackets);

        let result = worksheet.taxable_income(dec!(10000.00), dec!(15000.00));

        assert_eq!(result, dec!(0.00));
    }

    // =========================================================================
    // calculate_tax tests
    // =========================================================================

    #[test]
    fn calculate_tax_returns_zero_for_zero_income() {
        let brackets = test_brackets_single();
        let worksheet = EstimatedTaxWorksheet::new(&brackets);

        let result = worksheet.calculate_tax(dec!(0.00));

        assert_eq!(result, Ok(dec!(0.00)));
    }

    #[test]
    fn calculate_tax_first_bracket() {
        let brackets = test_brackets_single();
        let worksheet = EstimatedTaxWorksheet::new(&brackets);

        let result = worksheet.calculate_tax(dec!(10000.00));

        assert_eq!(result, Ok(dec!(1000.00)));
    }

    #[test]
    fn calculate_tax_second_bracket() {
        let brackets = test_brackets_single();
        let worksheet = EstimatedTaxWorksheet::new(&brackets);

        let result = worksheet.calculate_tax(dec!(30000.00));

        // Tax = 1192.50 + (30000 - 11925) * 0.12 = 1192.50 + 2169 = 3361.50
        assert_eq!(result, Ok(dec!(3361.50)));
    }

    #[test]
    fn calculate_tax_third_bracket() {
        let brackets = test_brackets_single();
        let worksheet = EstimatedTaxWorksheet::new(&brackets);

        let result = worksheet.calculate_tax(dec!(85000.00));

        // Tax = 5578.50 + (85000 - 48475) * 0.22 = 5578.50 + 8035.50 = 13614
        assert_eq!(result, Ok(dec!(13614.00)));
    }

    #[test]
    fn calculate_tax_highest_bracket() {
        let brackets = test_brackets_single();
        let worksheet = EstimatedTaxWorksheet::new(&brackets);

        let result = worksheet.calculate_tax(dec!(700000.00));

        // Tax = 188769.75 + (700000 - 626350) * 0.37 = 188769.75 + 27250.50 = 216020.25
        assert_eq!(result, Ok(dec!(216020.25)));
    }

    #[test]
    fn calculate_tax_returns_error_for_empty_brackets() {
        let brackets: Vec<TaxBracket> = vec![];
        let worksheet = EstimatedTaxWorksheet::new(&brackets);
        let input = test_input();

        let result = worksheet.calculate(&input);

        assert_eq!(result, Err(EstimatedTaxWorksheetError::NoTaxBrackets));
    }

    // =========================================================================
    // tax_after_credits tests
    // =========================================================================

    #[test]
    fn tax_after_credits_subtracts_credits() {
        let brackets = test_brackets_single();
        let worksheet = EstimatedTaxWorksheet::new(&brackets);

        let result = worksheet.tax_after_credits(dec!(14614.00), dec!(2000.00));

        assert_eq!(result, dec!(12614.00));
    }

    #[test]
    fn tax_after_credits_returns_zero_when_credits_exceed_tax() {
        let brackets = test_brackets_single();
        let worksheet = EstimatedTaxWorksheet::new(&brackets);

        let result = worksheet.tax_after_credits(dec!(1000.00), dec!(5000.00));

        assert_eq!(result, dec!(0.00));
    }

    // =========================================================================
    // current_year_factor tests
    // =========================================================================

    #[test]
    fn current_year_factor_applies_90_percent() {
        let brackets = test_brackets_single();
        let worksheet = EstimatedTaxWorksheet::new(&brackets);

        let result = worksheet.current_year_factor(dec!(10000.00), false);

        assert_eq!(result, dec!(9000.00));
    }

    #[test]
    fn current_year_factor_applies_two_thirds_for_farmer() {
        let brackets = test_brackets_single();
        let worksheet = EstimatedTaxWorksheet::new(&brackets);

        let result = worksheet.current_year_factor(dec!(10000.00), true);

        assert_eq!(result, dec!(6666.67));
    }

    // =========================================================================
    // required_annual_payment tests
    // =========================================================================

    #[test]
    fn required_annual_payment_returns_smaller_value() {
        let brackets = test_brackets_single();
        let worksheet = EstimatedTaxWorksheet::new(&brackets);

        let result = worksheet.required_annual_payment(dec!(9000.00), dec!(12000.00));

        assert_eq!(result, dec!(9000.00));
    }

    #[test]
    fn required_annual_payment_returns_prior_year_when_smaller() {
        let brackets = test_brackets_single();
        let worksheet = EstimatedTaxWorksheet::new(&brackets);

        let result = worksheet.required_annual_payment(dec!(15000.00), dec!(12000.00));

        assert_eq!(result, dec!(12000.00));
    }

    // =========================================================================
    // underpayment tests
    // =========================================================================

    #[test]
    fn underpayment_subtracts_withholding() {
        let brackets = test_brackets_single();
        let worksheet = EstimatedTaxWorksheet::new(&brackets);

        let result = worksheet.underpayment(dec!(12000.00), dec!(5000.00));

        assert_eq!(result, dec!(7000.00));
    }

    #[test]
    fn underpayment_returns_zero_when_withholding_exceeds() {
        let brackets = test_brackets_single();
        let worksheet = EstimatedTaxWorksheet::new(&brackets);

        let result = worksheet.underpayment(dec!(5000.00), dec!(10000.00));

        assert_eq!(result, dec!(0.00));
    }

    // =========================================================================
    // are_estimated_payments_required tests
    // =========================================================================

    #[test]
    fn payments_required_when_both_conditions_met() {
        let brackets = test_brackets_single();
        let worksheet = EstimatedTaxWorksheet::new(&brackets);

        let result =
            worksheet.are_estimated_payments_required(dec!(5000.00), dec!(2000.00), dec!(1000.00));

        assert!(result);
    }

    #[test]
    fn payments_not_required_when_underpayment_zero() {
        let brackets = test_brackets_single();
        let worksheet = EstimatedTaxWorksheet::new(&brackets);

        let result =
            worksheet.are_estimated_payments_required(dec!(0.00), dec!(2000.00), dec!(1000.00));

        assert!(!result);
    }

    #[test]
    fn payments_not_required_when_below_threshold() {
        let brackets = test_brackets_single();
        let worksheet = EstimatedTaxWorksheet::new(&brackets);

        let result =
            worksheet.are_estimated_payments_required(dec!(5000.00), dec!(500.00), dec!(1000.00));

        assert!(!result);
    }

    // =========================================================================
    // calculate (integration) tests
    // =========================================================================

    #[test]
    fn calculate_standard_case() {
        let brackets = test_brackets_single();
        let worksheet = EstimatedTaxWorksheet::new(&brackets);
        let input = test_input();

        let result = worksheet.calculate(&input).unwrap();

        // Taxable income: 100000 - 15000 = 85000
        assert_eq!(result.taxable_income, dec!(85000.00));
        // Tax: 5578.50 + (85000 - 48475) * 0.22 = 13614
        assert_eq!(result.calculated_tax, dec!(13614.00));
        assert_eq!(result.total_estimated_tax, dec!(13614.00));
        // Required: min(13614 * 0.90, 12000) = min(12252.60, 12000) = 12000
        assert_eq!(result.required_annual_payment, dec!(12000.00));
        // Underpayment: 12000 - 0 = 12000
        assert_eq!(result.underpayment, dec!(12000.00));
        assert!(!result.used_itemized_deduction);
        assert!(result.estimated_payments_required);
    }

    #[test]
    fn calculate_with_itemized_deduction() {
        let brackets = test_brackets_single();
        let worksheet = EstimatedTaxWorksheet::new(&brackets);
        let mut input = test_input();
        input.itemized_deduction = dec!(20000.00);

        let result = worksheet.calculate(&input).unwrap();

        // Taxable income: 100000 - 20000 = 80000
        assert_eq!(result.taxable_income, dec!(80000.00));
        assert!(result.used_itemized_deduction);
    }

    #[test]
    fn calculate_with_qbi_deduction() {
        let brackets = test_brackets_single();
        let worksheet = EstimatedTaxWorksheet::new(&brackets);
        let mut input = test_input();
        input.qbi_deduction = dec!(5000.00);

        let result = worksheet.calculate(&input).unwrap();

        // Taxable income: 100000 - 15000 - 5000 = 80000
        assert_eq!(result.taxable_income, dec!(80000.00));
    }

    #[test]
    fn calculate_with_withholding_covering_requirement() {
        let brackets = test_brackets_single();
        let worksheet = EstimatedTaxWorksheet::new(&brackets);
        let mut input = test_input();
        input.withholding = dec!(15000.00);

        let result = worksheet.calculate(&input).unwrap();

        assert_eq!(result.underpayment, dec!(0.00));
        assert!(!result.estimated_payments_required);
    }

    #[test]
    fn calculate_below_threshold_no_payments_required() {
        let brackets = test_brackets_single();
        let worksheet = EstimatedTaxWorksheet::new(&brackets);
        let mut input = test_input();
        input.adjusted_gross_income = dec!(30000.00);
        input.prior_year_tax = dec!(5000.00);
        input.withholding = dec!(1000.00);

        let result = worksheet.calculate(&input).unwrap();

        // Taxable income: 30000 - 15000 = 15000
        // Tax: 1192.50 + (15000 - 11925) * 0.12 = 1561.50
        // Threshold amount: 1561.50 - 1000 = 561.50 (below $1000)
        assert!(!result.estimated_payments_required);
    }

    #[test]
    fn calculate_farmer_uses_two_thirds_factor() {
        let brackets = test_brackets_single();
        let worksheet = EstimatedTaxWorksheet::new(&brackets);
        let mut input = test_input();
        input.is_farmer_or_fisher = true;
        input.prior_year_tax = dec!(20000.00);

        let result = worksheet.calculate(&input).unwrap();

        // Total tax: 13614
        // Current year factor: 13614 * (2/3) = 9076
        // Required: min(9076, 20000) = 9076
        assert_eq!(result.required_annual_payment, dec!(9076.00));
    }

    #[test]
    fn calculate_prior_year_smaller_uses_prior_year() {
        let brackets = test_brackets_single();
        let worksheet = EstimatedTaxWorksheet::new(&brackets);
        let mut input = test_input();
        input.prior_year_tax = dec!(8000.00);

        let result = worksheet.calculate(&input).unwrap();

        // Current year factor: 13614 * 0.90 = 12252.60
        // Required: min(12252.60, 8000) = 8000
        assert_eq!(result.required_annual_payment, dec!(8000.00));
        assert_eq!(result.underpayment, dec!(8000.00));
    }

    #[test]
    fn calculate_with_se_tax() {
        let brackets = test_brackets_single();
        let worksheet = EstimatedTaxWorksheet::new(&brackets);
        let mut input = test_input();
        input.self_employment_tax = dec!(7065.00);

        let result = worksheet.calculate(&input).unwrap();

        // Total estimated tax: 13614 + 7065 = 20679
        assert_eq!(result.total_estimated_tax, dec!(20679.00));
    }

    #[test]
    fn calculate_with_credits() {
        let brackets = test_brackets_single();
        let worksheet = EstimatedTaxWorksheet::new(&brackets);
        let mut input = test_input();
        input.credits = dec!(3000.00);

        let result = worksheet.calculate(&input).unwrap();

        // Total estimated tax: 13614 - 3000 = 10614
        assert_eq!(result.total_estimated_tax, dec!(10614.00));
    }

    #[test]
    fn calculate_credits_exceed_tax() {
        let brackets = test_brackets_single();
        let worksheet = EstimatedTaxWorksheet::new(&brackets);
        let mut input = test_input();
        input.adjusted_gross_income = dec!(50000.00);
        input.credits = dec!(10000.00);

        let result = worksheet.calculate(&input).unwrap();

        // Tax: about 3961.50, credits 10000 -> 0
        assert_eq!(result.total_estimated_tax, dec!(0.00));
    }

    #[test]
    fn calculate_low_income_no_tax() {
        let brackets = test_brackets_single();
        let worksheet = EstimatedTaxWorksheet::new(&brackets);
        let mut input = test_input();
        input.adjusted_gross_income = dec!(10000.00);

        let result = worksheet.calculate(&input).unwrap();

        // Taxable income: 10000 - 15000 = 0
        assert_eq!(result.taxable_income, dec!(0.00));
        assert_eq!(result.calculated_tax, dec!(0.00));
        assert_eq!(result.total_estimated_tax, dec!(0.00));
    }
}
