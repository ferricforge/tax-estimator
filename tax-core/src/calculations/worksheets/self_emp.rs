//! Self-Employment Tax Worksheet calculations for IRS Form 1040-ES.
//!
//! This module implements the Self-Employment Tax and Deduction Worksheet from
//! Form 1040-ES, which calculates self-employment tax and the deductible portion
//! of that tax.
//!
//! # Worksheet Structure
//!
//! The SE worksheet consists of the following lines:
//!
//! | Line | Description |
//! |------|-------------|
//! | 1a   | Expected income and profits subject to self-employment tax |
//! | 1b   | Conservation Reserve Program payments included on line 1a (if applicable) |
//! | 2    | Line 1a minus line 1b |
//! | 3    | Line 2 × 92.35% (net earnings factor) |
//! | 4    | Medicare tax: Line 3 × 2.9% |
//! | 5    | Maximum earnings subject to social security tax |
//! | 6    | Expected wages subject to social security tax (or 6.2% tier 1 RRTA) |
//! | 7    | Line 5 minus Line 6 (if zero or less, skip to Line 10) |
//! | 8    | Smaller of Line 3 or Line 7 |
//! | 9    | Social security tax: Line 8 × 12.4% |
//! | 10   | Self-employment tax: Line 4 + Line 9 |
//! | 11   | Deductible part of SE tax: Line 10 × 50% |
//!
//! # Minimum Threshold
//!
//! If net earnings from self-employment (Line 3) are less than $400, no
//! self-employment tax is due and Schedule SE is not required. This threshold is
//! configurable via [`SeWorksheetConfig::min_se_threshold`]. The 1040-ES worksheet
//! does not state it; it comes from the Schedule SE instructions.
//!
//! # Example
//!
//! ```
//! use rust_decimal_macros::dec;
//! use tax_core::calculations::{SeWorksheet, SeWorksheetConfig};
//!
//! let config = SeWorksheetConfig {
//!     ss_wage_max: dec!(176100.00),
//!     ss_tax_rate: dec!(0.124),
//!     medicare_tax_rate: dec!(0.029),
//!     net_earnings_factor: dec!(0.9235),
//!     deduction_factor: dec!(0.50),
//!     min_se_threshold: dec!(400.00),
//! };
//!
//! let worksheet = SeWorksheet::new(config);
//! let result = worksheet.calculate(
//!     dec!(100000.00),  // se_income
//!     dec!(0.00),       // crp_payments
//!     dec!(50000.00),   // wages
//! ).unwrap();
//!
//! assert_eq!(result.self_employment_tax, dec!(14129.55));
//! assert_eq!(result.se_tax_deduction, dec!(7064.78));
//! ```
use std::fmt;

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::warn;

use crate::TaxYearConfig;
use crate::calculations::common::round_half_up;

/// Errors that can occur during SE worksheet calculations.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SeWorksheetError {
    /// The net earnings factor must be between 0 and 1 (exclusive of 0).
    #[error("net earnings factor must be between 0 and 1, got {0}")]
    InvalidNetEarningsFactor(Decimal),
    /// The social security tax rate must be between 0 and 1.
    #[error("social security tax rate must be between 0 and 1, got {0}")]
    InvalidSocialSecurityRate(Decimal),
    /// The Medicare tax rate must be between 0 and 1.
    #[error("medicare tax rate must be between 0 and 1, got {0}")]
    InvalidMedicareRate(Decimal),
    /// The deduction factor must be between 0 and 1.
    #[error("deduction factor must be between 0 and 1, got {0}")]
    InvalidDeductionFactor(Decimal),
    /// The social security wage maximum must be positive.
    #[error("social security wage maximum must be positive, got {0}")]
    InvalidSsWageMax(Decimal),
    /// The minimum SE threshold must be non-negative.
    #[error("minimum SE threshold must be non-negative, got {0}")]
    InvalidMinSeThreshold(Decimal),
}

/// Configuration parameters for SE worksheet calculations.
///
/// These values are typically obtained from [`TaxYearConfig`] and represent
/// IRS-specified rates and limits that may change from year to year.
///
/// # Example
///
/// ```
/// use rust_decimal_macros::dec;
/// use tax_core::calculations::SeWorksheetConfig;
///
/// // 2025 tax year configuration
/// let config = SeWorksheetConfig {
///     ss_wage_max: dec!(176100.00),
///     ss_tax_rate: dec!(0.124),
///     medicare_tax_rate: dec!(0.029),
///     net_earnings_factor: dec!(0.9235),
///     deduction_factor: dec!(0.50),
///     min_se_threshold: dec!(400.00),
/// };
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeWorksheetConfig {
    /// Maximum earnings subject to social security tax (Line 5).
    ///
    /// For 2025, this is $176,100.
    pub ss_wage_max: Decimal,
    /// Combined social security tax rate for self-employment (Line 9 multiplier).
    ///
    /// This is the combined employer and employee portions, typically 12.4%.
    pub ss_tax_rate: Decimal,
    /// Combined Medicare tax rate for self-employment (Line 4 multiplier).
    ///
    /// This is the combined employer and employee portions, typically 2.9%.
    pub medicare_tax_rate: Decimal,
    /// Factor applied to net earnings to calculate taxable amount (Line 3 multiplier).
    ///
    /// This represents the portion of self-employment income that is subject
    /// to SE tax after the "employer-equivalent" adjustment. Typically 92.35%.
    pub net_earnings_factor: Decimal,
    /// Factor for calculating the deductible portion of SE tax (Line 11 multiplier).
    ///
    /// This represents the "employer-equivalent" portion of SE tax that is
    /// deductible. Typically 50%.
    pub deduction_factor: Decimal,
    /// Minimum net earnings (Line 3) for SE tax to apply.
    ///
    /// If net earnings from self-employment are less than this amount, no SE tax
    /// is due. For 2025, this is $400.
    pub min_se_threshold: Decimal,
}

impl SeWorksheetConfig {
    /// Creates a new configuration from a [`TaxYearConfig`].
    ///
    /// # Example
    ///
    /// ```
    /// use rust_decimal_macros::dec;
    /// use tax_core::{TaxYearConfig, calculations::SeWorksheetConfig};
    ///
    /// let tax_year_config = TaxYearConfig {
    ///     tax_year: 2025,
    ///     ss_wage_max: dec!(176100.00),
    ///     ss_tax_rate: dec!(0.124),
    ///     medicare_tax_rate: dec!(0.029),
    ///     se_tax_deduct_pcnt: dec!(0.9235),
    ///     se_deduction_factor: dec!(0.50),
    ///     req_pmnt_threshold: dec!(1000.00),
    ///     min_se_threshold: dec!(400.00),
    /// };
    ///
    /// let config = SeWorksheetConfig::from_tax_year_config(&tax_year_config);
    ///
    /// assert_eq!(config.ss_wage_max, dec!(176100.00));
    /// assert_eq!(config.ss_tax_rate, dec!(0.124));
    /// assert_eq!(config.min_se_threshold, dec!(400.00));
    /// ```
    pub fn from_tax_year_config(config: &TaxYearConfig) -> Self {
        Self {
            ss_wage_max: config.ss_wage_max,
            ss_tax_rate: config.ss_tax_rate,
            medicare_tax_rate: config.medicare_tax_rate,
            net_earnings_factor: config.se_tax_deduct_pcnt,
            deduction_factor: config.se_deduction_factor,
            min_se_threshold: config.min_se_threshold,
        }
    }

    /// Validates the configuration values.
    ///
    /// Returns an error if any configuration value is outside its valid range.
    ///
    /// # Errors
    ///
    /// Returns [`SeWorksheetError`] if:
    /// - `net_earnings_factor` is not in (0, 1]
    /// - `ss_tax_rate` is not in [0, 1]
    /// - `medicare_tax_rate` is not in [0, 1]
    /// - `deduction_factor` is not in [0, 1]
    /// - `ss_wage_max` is not positive
    /// - `min_se_threshold` is negative
    ///
    /// # Example
    ///
    /// ```
    /// use rust_decimal_macros::dec;
    /// use tax_core::calculations::{SeWorksheetConfig, SeWorksheetError};
    ///
    /// let invalid_config = SeWorksheetConfig {
    ///     ss_wage_max: dec!(-1000.00),
    ///     ss_tax_rate: dec!(0.124),
    ///     medicare_tax_rate: dec!(0.029),
    ///     net_earnings_factor: dec!(0.9235),
    ///     deduction_factor: dec!(0.50),
    ///     min_se_threshold: dec!(400.00),
    /// };
    ///
    /// let result = invalid_config.validate();
    /// assert_eq!(result, Err(SeWorksheetError::InvalidSsWageMax(dec!(-1000.00))));
    /// ```
    pub fn validate(&self) -> Result<(), SeWorksheetError> {
        if self.net_earnings_factor <= Decimal::ZERO || self.net_earnings_factor > Decimal::ONE {
            return Err(SeWorksheetError::InvalidNetEarningsFactor(
                self.net_earnings_factor,
            ));
        }
        if !is_fraction(self.ss_tax_rate) {
            return Err(SeWorksheetError::InvalidSocialSecurityRate(
                self.ss_tax_rate,
            ));
        }
        if !is_fraction(self.medicare_tax_rate) {
            return Err(SeWorksheetError::InvalidMedicareRate(
                self.medicare_tax_rate,
            ));
        }
        if !is_fraction(self.deduction_factor) {
            return Err(SeWorksheetError::InvalidDeductionFactor(
                self.deduction_factor,
            ));
        }
        if self.ss_wage_max <= Decimal::ZERO {
            return Err(SeWorksheetError::InvalidSsWageMax(self.ss_wage_max));
        }
        if self.min_se_threshold < Decimal::ZERO {
            return Err(SeWorksheetError::InvalidMinSeThreshold(
                self.min_se_threshold,
            ));
        }
        Ok(())
    }
}

/// Returns `true` when `value` is a fraction from 0 to 1, inclusive.
fn is_fraction(value: Decimal) -> bool {
    (Decimal::ZERO..=Decimal::ONE).contains(&value)
}

/// Result of SE worksheet calculations.
///
/// Contains both the self-employment tax amount and the deductible portion
/// of that tax, along with intermediate calculation values for transparency.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeWorksheetResult {
    /// Line 2: line 1a minus line 1b, before the net earnings factor.
    pub combined_se_income: Decimal,
    /// Net earnings from self-employment after applying the net earnings factor (Line 3).
    ///
    /// This is line 2 × 92.35% (or configured factor).
    pub net_earnings: Decimal,
    /// Medicare tax component (Line 4).
    ///
    /// Calculated as net_earnings × 2.9% (or configured rate).
    pub medicare_tax: Decimal,
    /// Remaining social security wage base after subtracting wages (Line 7).
    ///
    /// Calculated as `ss_wage_max − wages`, clamped to zero. When wages meet
    /// or exceed the SS wage maximum, no social security tax applies to SE
    /// earnings and this value is zero.
    pub remaining_ss_base: Decimal,
    /// Earnings subject to social security tax (Line 8).
    ///
    /// This is the smaller of net earnings or the remaining SS wage base
    /// after accounting for wages.
    pub ss_taxable_earnings: Decimal,
    /// Social security tax component (Line 9).
    ///
    /// Calculated as ss_taxable_earnings × 12.4% (or configured rate).
    pub social_security_tax: Decimal,
    /// Total self-employment tax (Line 10).
    ///
    /// This is medicare_tax + social_security_tax.
    pub self_employment_tax: Decimal,
    /// Deductible portion of self-employment tax (Line 11).
    ///
    /// Calculated as self_employment_tax × 50% (or configured factor).
    /// This amount is entered on Schedule 1, Line 15.
    pub se_tax_deduction: Decimal,
    /// Indicates whether SE tax was skipped due to net earnings below the threshold.
    ///
    /// If `true`, net earnings (Line 3) were less than the minimum threshold
    /// (typically $400), so no SE tax is due and Lines 4 through 11 are zero.
    pub below_threshold: bool,
}

impl SeWorksheetResult {
    /// Creates a zero-valued result for net earnings below the SE threshold.
    ///
    /// Lines 4 through 11 do not apply when Schedule SE is not required, so they
    /// are reported as zero.
    fn below_threshold(combined_se_income: Decimal) -> Self {
        Self {
            combined_se_income,
            net_earnings: Decimal::ZERO,
            medicare_tax: Decimal::ZERO,
            remaining_ss_base: Decimal::ZERO,
            ss_taxable_earnings: Decimal::ZERO,
            social_security_tax: Decimal::ZERO,
            self_employment_tax: Decimal::ZERO,
            se_tax_deduction: Decimal::ZERO,
            below_threshold: true,
        }
    }
}

impl fmt::Display for SeWorksheetResult {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        writeln!(f, "SeWorksheetResult {{")?;
        writeln!(
            f,
            "    combined_se_income   : ${}",
            self.combined_se_income.round_dp(2)
        )?;
        writeln!(
            f,
            "    net_earnings        : ${}",
            self.net_earnings.round_dp(2)
        )?;
        writeln!(
            f,
            "    medicare_tax        : ${}",
            self.medicare_tax.round_dp(2)
        )?;
        writeln!(
            f,
            "    remaining_ss_base   : ${}",
            self.remaining_ss_base.round_dp(2)
        )?;
        writeln!(
            f,
            "    ss_taxable_earnings : ${}",
            self.ss_taxable_earnings.round_dp(2)
        )?;
        writeln!(
            f,
            "    social_security_tax : ${}",
            self.social_security_tax.round_dp(2)
        )?;
        writeln!(
            f,
            "    self_employment_tax : ${}",
            self.self_employment_tax.round_dp(2)
        )?;
        writeln!(
            f,
            "    se_tax_deduction    : ${}",
            self.se_tax_deduction.round_dp(2)
        )?;
        writeln!(f, "    below_threshold    : {}", self.below_threshold)?;
        write!(f, "}}")?;
        Ok(())
    }
}

/// Calculator for the Self-Employment Tax Worksheet.
///
/// This struct encapsulates the configuration and provides methods to calculate
/// each line of the SE worksheet, culminating in the total SE tax and deduction.
///
/// # Example
///
/// ```
/// use rust_decimal_macros::dec;
/// use tax_core::calculations::{SeWorksheet, SeWorksheetConfig};
///
/// let config = SeWorksheetConfig {
///     ss_wage_max: dec!(176100.00),
///     ss_tax_rate: dec!(0.124),
///     medicare_tax_rate: dec!(0.029),
///     net_earnings_factor: dec!(0.9235),
///     deduction_factor: dec!(0.50),
///     min_se_threshold: dec!(400.00),
/// };
///
/// let worksheet = SeWorksheet::new(config);
///
/// // Calculate SE tax for $100,000 in SE income with no wages
/// let result = worksheet.calculate(dec!(100000.00), dec!(0.00), dec!(0.00)).unwrap();
///
/// // Net earnings = $100,000 × 0.9235 = $92,350
/// assert_eq!(result.net_earnings, dec!(92350.00));
/// ```
#[derive(Debug, Clone)]
pub struct SeWorksheet {
    config: SeWorksheetConfig,
}

impl SeWorksheet {
    /// Creates a new SE worksheet calculator with the given configuration.
    ///
    /// # Example
    ///
    /// ```
    /// use rust_decimal_macros::dec;
    /// use tax_core::calculations::{SeWorksheet, SeWorksheetConfig};
    ///
    /// let config = SeWorksheetConfig {
    ///     ss_wage_max: dec!(176100.00),
    ///     ss_tax_rate: dec!(0.124),
    ///     medicare_tax_rate: dec!(0.029),
    ///     net_earnings_factor: dec!(0.9235),
    ///     deduction_factor: dec!(0.50),
    ///     min_se_threshold: dec!(400.00),
    /// };
    ///
    /// let worksheet = SeWorksheet::new(config);
    /// ```
    pub fn new(config: SeWorksheetConfig) -> Self {
        Self { config }
    }

    /// Calculates the complete SE worksheet and returns the result.
    ///
    /// This is the main entry point for SE tax calculations. It validates
    /// the configuration, checks the minimum threshold, performs all line
    /// calculations, and returns a comprehensive result.
    ///
    /// # Arguments
    ///
    /// * `se_income` - Expected income and profits subject to SE tax (Line 1a)
    /// * `crp_payments` - Conservation Reserve Program payments (Line 1b)
    /// * `wages` - Total wages subject to social security tax (Line 6)
    ///
    /// # Returns
    ///
    /// Returns [`SeWorksheetResult`] containing the calculated SE tax and
    /// deduction, along with intermediate values for Lines 2, 3, 4, 7, 8, 9,
    /// 10, and 11. If net earnings (Line 3) are less than the minimum threshold
    /// ($400 for 2025), returns a zero-valued result with `below_threshold` set
    /// to `true`.
    ///
    /// # Errors
    ///
    /// Returns [`SeWorksheetError`] if the configuration is invalid.
    ///
    /// # Example
    ///
    /// ```
    /// use rust_decimal_macros::dec;
    /// use tax_core::calculations::{SeWorksheet, SeWorksheetConfig};
    ///
    /// let config = SeWorksheetConfig {
    ///     ss_wage_max: dec!(176100.00),
    ///     ss_tax_rate: dec!(0.124),
    ///     medicare_tax_rate: dec!(0.029),
    ///     net_earnings_factor: dec!(0.9235),
    ///     deduction_factor: dec!(0.50),
    ///     min_se_threshold: dec!(400.00),
    /// };
    ///
    /// let worksheet = SeWorksheet::new(config);
    ///
    /// // Self-employed with $80,000 SE income and $60,000 in wages
    /// let result = worksheet.calculate(
    ///     dec!(80000.00),
    ///     dec!(0.00),
    ///     dec!(60000.00),
    /// ).unwrap();
    ///
    /// // Net earnings = $80,000 × 0.9235 = $73,880
    /// assert_eq!(result.net_earnings, dec!(73880.00));
    ///
    /// // Remaining SS base = $176,100 - $60,000 = $116,100
    /// assert_eq!(result.remaining_ss_base, dec!(116100.00));
    ///
    /// // SS taxable = min($73,880, $116,100) = $73,880
    /// assert_eq!(result.ss_taxable_earnings, dec!(73880.00));
    /// ```
    ///
    /// # Example: Below Threshold
    ///
    /// ```
    /// use rust_decimal_macros::dec;
    /// use tax_core::calculations::{SeWorksheet, SeWorksheetConfig};
    ///
    /// let config = SeWorksheetConfig {
    ///     ss_wage_max: dec!(176100.00),
    ///     ss_tax_rate: dec!(0.124),
    ///     medicare_tax_rate: dec!(0.029),
    ///     net_earnings_factor: dec!(0.9235),
    ///     deduction_factor: dec!(0.50),
    ///     min_se_threshold: dec!(400.00),
    /// };
    ///
    /// let worksheet = SeWorksheet::new(config);
    ///
    /// // $400 of SE income gives net earnings of $369.40, below the $400 threshold
    /// let result = worksheet.calculate(dec!(400.00), dec!(0.00), dec!(0.00)).unwrap();
    ///
    /// assert!(result.below_threshold);
    /// assert_eq!(result.self_employment_tax, dec!(0.00));
    /// ```
    pub fn calculate(
        &self,
        se_income: Decimal,
        crp_payments: Decimal,
        wages: Decimal,
    ) -> Result<SeWorksheetResult, SeWorksheetError> {
        self.config.validate()?;

        // Line 2: Line 1a minus Line 1b
        let line_2 = self.line_2_se_income(se_income, crp_payments);

        // Line 3: Net earnings from self-employment
        let net_earnings = self.net_earnings_from_self_employment(line_2);

        // Schedule SE is not required below the threshold, so Lines 4 through 11 do not apply
        if net_earnings < self.config.min_se_threshold {
            warn!(
                net_earnings = %net_earnings,
                threshold = %self.config.min_se_threshold,
                "Net earnings below minimum threshold; no SE tax due"
            );
            return Ok(SeWorksheetResult::below_threshold(line_2));
        }

        // Line 4: Medicare tax
        let medicare_tax = self.medicare_tax(net_earnings);

        // Line 7: Remaining SS wage base after wages
        let remaining_ss_base = self.remaining_ss_wage_base(wages);

        // Line 8: SS taxable earnings (smaller of Line 3 or Line 7)
        let ss_taxable_earnings = self.ss_taxable_earnings(net_earnings, remaining_ss_base);

        // Line 9: Social security tax
        let social_security_tax = self.social_security_tax(ss_taxable_earnings);

        // Line 10: Total self-employment tax
        let self_employment_tax = self.total_self_employment_tax(medicare_tax, social_security_tax);

        // Line 11: SE tax deduction
        let se_tax_deduction = self.se_tax_deduction(self_employment_tax);

        Ok(SeWorksheetResult {
            combined_se_income: line_2,
            net_earnings,
            medicare_tax,
            remaining_ss_base,
            ss_taxable_earnings,
            social_security_tax,
            self_employment_tax,
            se_tax_deduction,
            below_threshold: false,
        })
    }

    /// Calculates Line 2: expected income (Line 1a) minus CRP payments (Line 1b).
    ///
    /// Line 1b applies only when the taxpayer has farm income and also receives
    /// social security retirement or disability benefits. The worksheet removes
    /// those payments from the SE base, so they are subtracted here.
    ///
    /// # Form Reference
    ///
    /// Line 2: Subtract line 1b from line 1a
    fn line_2_se_income(
        &self,
        se_income: Decimal,
        crp_payments: Decimal,
    ) -> Decimal {
        let line_2 = se_income - crp_payments;

        if line_2 < Decimal::ZERO {
            warn!(
                se_income = %se_income,
                crp_payments = %crp_payments,
                line_2 = %line_2,
                "Line 2 is negative; no SE tax will be due"
            );
        }

        round_half_up(line_2)
    }

    /// Calculates net earnings from self-employment (Line 3).
    ///
    /// Multiplies line 2 by the net earnings factor (typically 92.35%) to arrive
    /// at the amount subject to SE tax calculation.
    ///
    /// # Form Reference
    ///
    /// Line 3: Multiply line 2 by 92.35% (0.9235)
    fn net_earnings_from_self_employment(
        &self,
        line_2: Decimal,
    ) -> Decimal {
        let net_earnings = line_2 * self.config.net_earnings_factor;
        let rounded = round_half_up(net_earnings);

        // If negative, SE tax is zero but we return the value for transparency
        if rounded < Decimal::ZERO {
            warn!(
                line_2 = %line_2,
                net_earnings_factor = %self.config.net_earnings_factor,
                net_earnings = %rounded,
                "Net earnings from self-employment is negative"
            );
        }

        rounded
    }

    /// Calculates Medicare tax on net SE earnings (Line 4).
    ///
    /// Multiplies net earnings by the Medicare tax rate (typically 2.9%).
    /// Unlike social security tax, Medicare tax applies to all net earnings
    /// without a wage base limit.
    ///
    /// # Form Reference
    ///
    /// Line 4: Multiply line 3 by 2.9% (0.029)
    fn medicare_tax(
        &self,
        net_earnings: Decimal,
    ) -> Decimal {
        if net_earnings <= Decimal::ZERO {
            warn!(
                net_earnings = %net_earnings,
                "Net earnings are zero or negative; no Medicare tax applies"
            );
            return Decimal::ZERO;
        }

        let tax = net_earnings * self.config.medicare_tax_rate;
        round_half_up(tax)
    }

    /// Calculates the remaining social security wage base (Line 7).
    ///
    /// Subtracts wages already subject to social security tax from the maximum
    /// wage base. If wages meet or exceed the maximum, returns zero.
    ///
    /// # Form Reference
    ///
    /// - Line 5: Maximum income subject to social security tax
    /// - Line 6: Total wages subject to social security tax
    /// - Line 7: Line 5 minus Line 6 (if zero or less, skip to Line 10)
    fn remaining_ss_wage_base(
        &self,
        wages: Decimal,
    ) -> Decimal {
        let remaining = self.config.ss_wage_max - wages;

        if remaining <= Decimal::ZERO {
            warn!(
                ss_wage_max = %self.config.ss_wage_max,
                wages = %wages,
                "Wages exceed or equal SS wage maximum; no SS tax on SE income"
            );
            return Decimal::ZERO;
        }

        round_half_up(remaining)
    }

    /// Determines earnings subject to social security tax (Line 8).
    ///
    /// Returns the smaller of net earnings or the remaining SS wage base.
    /// If net earnings are negative, returns zero.
    ///
    /// # Form Reference
    ///
    /// Line 8: Enter the smaller of line 3 or line 7
    fn ss_taxable_earnings(
        &self,
        net_earnings: Decimal,
        remaining_ss_base: Decimal,
    ) -> Decimal {
        if net_earnings <= Decimal::ZERO {
            warn!(
                net_earnings = %net_earnings,
                "Net earnings are zero or negative; no SS tax applies"
            );
            return Decimal::ZERO;
        }

        round_half_up(net_earnings.min(remaining_ss_base))
    }

    /// Calculates social security tax on SE earnings (Line 9).
    ///
    /// Multiplies the SS taxable earnings by the social security tax rate
    /// (typically 12.4%).
    ///
    /// # Form Reference
    ///
    /// Line 9: Multiply line 8 by 12.4% (0.124)
    fn social_security_tax(
        &self,
        ss_taxable_earnings: Decimal,
    ) -> Decimal {
        let tax = ss_taxable_earnings * self.config.ss_tax_rate;
        round_half_up(tax)
    }

    /// Calculates total self-employment tax (Line 10).
    ///
    /// Adds the Medicare tax and social security tax components.
    ///
    /// # Form Reference
    ///
    /// Line 10: Add lines 4 and 9
    fn total_self_employment_tax(
        &self,
        medicare_tax: Decimal,
        social_security_tax: Decimal,
    ) -> Decimal {
        round_half_up(medicare_tax + social_security_tax)
    }

    /// Calculates the deductible portion of SE tax (Line 11).
    ///
    /// Multiplies the total SE tax by the deduction factor (typically 50%)
    /// to determine the amount that can be deducted on Schedule 1.
    ///
    /// # Form Reference
    ///
    /// Line 11: Multiply line 10 by 50% (0.50). Enter the result here and
    /// on Schedule 1 (Form 1040), line 15.
    fn se_tax_deduction(
        &self,
        self_employment_tax: Decimal,
    ) -> Decimal {
        let deduction = self_employment_tax * self.config.deduction_factor;
        round_half_up(deduction)
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use rust_decimal_macros::dec;

    use super::*;

    /// Creates a standard 2025 tax year configuration for testing.
    fn test_config() -> SeWorksheetConfig {
        SeWorksheetConfig {
            ss_wage_max: dec!(176100.00),
            ss_tax_rate: dec!(0.124),
            medicare_tax_rate: dec!(0.029),
            net_earnings_factor: dec!(0.9235),
            deduction_factor: dec!(0.50),
            min_se_threshold: dec!(400.00),
        }
    }

    fn worksheet() -> SeWorksheet {
        SeWorksheet::new(test_config())
    }

    // =========================================================================
    // SeWorksheetConfig::validate
    // =========================================================================

    #[test]
    fn validate_accepts_valid_config() {
        let result = test_config().validate();

        assert_eq!(result, Ok(()));
    }

    #[test]
    fn validate_rejects_zero_net_earnings_factor() {
        let config = SeWorksheetConfig {
            net_earnings_factor: dec!(0.00),
            ..test_config()
        };

        let result = config.validate();

        assert_eq!(
            result,
            Err(SeWorksheetError::InvalidNetEarningsFactor(dec!(0.00)))
        );
    }

    #[test]
    fn validate_rejects_net_earnings_factor_above_one() {
        let config = SeWorksheetConfig {
            net_earnings_factor: dec!(1.5),
            ..test_config()
        };

        let result = config.validate();

        assert_eq!(
            result,
            Err(SeWorksheetError::InvalidNetEarningsFactor(dec!(1.5)))
        );
    }

    #[test]
    fn validate_accepts_net_earnings_factor_of_one() {
        let config = SeWorksheetConfig {
            net_earnings_factor: dec!(1.00),
            ..test_config()
        };

        let result = config.validate();

        assert_eq!(result, Ok(()));
    }

    #[test]
    fn validate_rejects_negative_ss_tax_rate() {
        let config = SeWorksheetConfig {
            ss_tax_rate: dec!(-0.1),
            ..test_config()
        };

        let result = config.validate();

        assert_eq!(
            result,
            Err(SeWorksheetError::InvalidSocialSecurityRate(dec!(-0.1)))
        );
    }

    #[test]
    fn validate_rejects_ss_tax_rate_above_one() {
        let config = SeWorksheetConfig {
            ss_tax_rate: dec!(1.5),
            ..test_config()
        };

        let result = config.validate();

        assert_eq!(
            result,
            Err(SeWorksheetError::InvalidSocialSecurityRate(dec!(1.5)))
        );
    }

    #[test]
    fn validate_rejects_negative_medicare_rate() {
        let config = SeWorksheetConfig {
            medicare_tax_rate: dec!(-0.1),
            ..test_config()
        };

        let result = config.validate();

        assert_eq!(
            result,
            Err(SeWorksheetError::InvalidMedicareRate(dec!(-0.1)))
        );
    }

    #[test]
    fn validate_rejects_medicare_rate_above_one() {
        let config = SeWorksheetConfig {
            medicare_tax_rate: dec!(1.5),
            ..test_config()
        };

        let result = config.validate();

        assert_eq!(
            result,
            Err(SeWorksheetError::InvalidMedicareRate(dec!(1.5)))
        );
    }

    #[test]
    fn validate_rejects_negative_deduction_factor() {
        let config = SeWorksheetConfig {
            deduction_factor: dec!(-0.1),
            ..test_config()
        };

        let result = config.validate();

        assert_eq!(
            result,
            Err(SeWorksheetError::InvalidDeductionFactor(dec!(-0.1)))
        );
    }

    #[test]
    fn validate_rejects_deduction_factor_above_one() {
        let config = SeWorksheetConfig {
            deduction_factor: dec!(1.5),
            ..test_config()
        };

        let result = config.validate();

        assert_eq!(
            result,
            Err(SeWorksheetError::InvalidDeductionFactor(dec!(1.5)))
        );
    }

    #[test]
    fn validate_rejects_zero_ss_wage_max() {
        let config = SeWorksheetConfig {
            ss_wage_max: dec!(0.00),
            ..test_config()
        };

        let result = config.validate();

        assert_eq!(result, Err(SeWorksheetError::InvalidSsWageMax(dec!(0.00))));
    }

    #[test]
    fn validate_rejects_negative_min_se_threshold() {
        let config = SeWorksheetConfig {
            min_se_threshold: dec!(-100.00),
            ..test_config()
        };

        let result = config.validate();

        assert_eq!(
            result,
            Err(SeWorksheetError::InvalidMinSeThreshold(dec!(-100.00)))
        );
    }

    #[test]
    fn validate_accepts_zero_min_se_threshold() {
        let config = SeWorksheetConfig {
            min_se_threshold: dec!(0.00),
            ..test_config()
        };

        let result = config.validate();

        assert_eq!(result, Ok(()));
    }

    // =========================================================================
    // SeWorksheetConfig::from_tax_year_config
    // =========================================================================

    #[test]
    fn from_tax_year_config_maps_every_field() {
        let tax_year_config = TaxYearConfig {
            tax_year: 2025,
            ss_wage_max: dec!(176100.00),
            ss_tax_rate: dec!(0.124),
            medicare_tax_rate: dec!(0.029),
            se_tax_deduct_pcnt: dec!(0.9235),
            se_deduction_factor: dec!(0.50),
            req_pmnt_threshold: dec!(1000.00),
            min_se_threshold: dec!(400.00),
        };

        let config = SeWorksheetConfig::from_tax_year_config(&tax_year_config);

        assert_eq!(config, test_config());
    }

    // =========================================================================
    // Line 2: line_2_se_income
    // =========================================================================

    #[test]
    fn line_2_subtracts_crp_payments_from_se_income() {
        let result = worksheet().line_2_se_income(dec!(50000.00), dec!(5000.00));

        assert_eq!(result, dec!(45000.00));
    }

    #[test]
    fn line_2_rounds_half_up_to_cents() {
        let result = worksheet().line_2_se_income(dec!(300.126), dec!(0.00));

        assert_eq!(result, dec!(300.13));
    }

    // =========================================================================
    // Line 3: net_earnings_from_self_employment
    // =========================================================================

    #[test]
    fn net_earnings_rounds_half_up_to_cents() {
        // 12345.67 × 0.9235 = 11401.226245
        let result = worksheet().net_earnings_from_self_employment(dec!(12345.67));

        assert_eq!(result, dec!(11401.23));
    }

    // =========================================================================
    // Line 4: medicare_tax
    // =========================================================================

    #[test]
    fn medicare_tax_has_no_wage_cap() {
        let result = worksheet().medicare_tax(dec!(500000.00));

        assert_eq!(result, dec!(14500.00));
    }

    #[test]
    fn medicare_tax_is_zero_for_negative_earnings() {
        let result = worksheet().medicare_tax(dec!(-5000.00));

        assert_eq!(result, dec!(0.00));
    }

    // =========================================================================
    // Line 7: remaining_ss_wage_base
    // =========================================================================

    #[test]
    fn remaining_ss_base_is_zero_when_wages_equal_maximum() {
        let result = worksheet().remaining_ss_wage_base(dec!(176100.00));

        assert_eq!(result, dec!(0.00));
    }

    // =========================================================================
    // Line 8: ss_taxable_earnings
    // =========================================================================

    #[test]
    fn ss_taxable_earnings_is_zero_for_negative_net_earnings() {
        let result = worksheet().ss_taxable_earnings(dec!(-5000.00), dec!(100000.00));

        assert_eq!(result, dec!(0.00));
    }

    // =========================================================================
    // calculate: end-to-end scenarios
    // =========================================================================

    #[test]
    fn calculate_returns_every_line_for_standard_case() {
        let result = worksheet()
            .calculate(dec!(100000.00), dec!(0.00), dec!(50000.00))
            .unwrap();

        assert_eq!(
            result,
            SeWorksheetResult {
                combined_se_income: dec!(100000.00),
                net_earnings: dec!(92350.00),
                medicare_tax: dec!(2678.15),
                remaining_ss_base: dec!(126100.00),
                ss_taxable_earnings: dec!(92350.00),
                social_security_tax: dec!(11451.40),
                self_employment_tax: dec!(14129.55),
                se_tax_deduction: dec!(7064.78),
                below_threshold: false,
            }
        );
    }

    #[test]
    fn calculate_subtracts_crp_payments_before_the_net_earnings_factor() {
        let result = worksheet()
            .calculate(dec!(100000.00), dec!(20000.00), dec!(0.00))
            .unwrap();

        // Line 2: 100000 − 20000 = 80000; Line 3: 80000 × 0.9235 = 73880
        assert_eq!(result.combined_se_income, dec!(80000.00));
        assert_eq!(result.net_earnings, dec!(73880.00));
        assert_eq!(result.medicare_tax, dec!(2142.52));
        assert_eq!(result.social_security_tax, dec!(9161.12));
        assert_eq!(result.self_employment_tax, dec!(11303.64));
        assert_eq!(result.se_tax_deduction, dec!(5651.82));
    }

    #[test]
    fn calculate_skips_social_security_when_wages_exceed_maximum() {
        let result = worksheet()
            .calculate(dec!(50000.00), dec!(0.00), dec!(200000.00))
            .unwrap();

        // Line 3: 50000 × 0.9235 = 46175; Line 4: 46175 × 0.029 = 1339.075 → 1339.08
        assert_eq!(result.net_earnings, dec!(46175.00));
        assert_eq!(result.medicare_tax, dec!(1339.08));
        assert_eq!(result.remaining_ss_base, dec!(0.00));
        assert_eq!(result.ss_taxable_earnings, dec!(0.00));
        assert_eq!(result.social_security_tax, dec!(0.00));
        assert_eq!(result.self_employment_tax, dec!(1339.08));
        assert_eq!(result.se_tax_deduction, dec!(669.54));
    }

    #[test]
    fn calculate_caps_social_security_earnings_at_wage_base() {
        let result = worksheet()
            .calculate(dec!(250000.00), dec!(0.00), dec!(0.00))
            .unwrap();

        // Line 3: 250000 × 0.9235 = 230875 (above the 176100 base)
        assert_eq!(result.net_earnings, dec!(230875.00));
        assert_eq!(result.medicare_tax, dec!(6695.38));
        assert_eq!(result.ss_taxable_earnings, dec!(176100.00));
        assert_eq!(result.social_security_tax, dec!(21836.40));
        assert_eq!(result.self_employment_tax, dec!(28531.78));
        assert_eq!(result.se_tax_deduction, dec!(14265.89));
    }

    #[test]
    fn calculate_skips_tax_when_net_earnings_are_just_below_threshold() {
        // 433.12 × 0.9235 = 399.986 → 399.99, below $400
        let result = worksheet()
            .calculate(dec!(433.12), dec!(0.00), dec!(0.00))
            .unwrap();

        assert!(result.below_threshold);
        assert_eq!(result.combined_se_income, dec!(433.12));
        assert_eq!(result.net_earnings, dec!(0.00));
        assert_eq!(result.self_employment_tax, dec!(0.00));
        assert_eq!(result.se_tax_deduction, dec!(0.00));
    }

    #[test]
    fn calculate_taxes_net_earnings_exactly_at_threshold() {
        // 433.13 × 0.9235 = 399.995555 → 400.00, which is not below $400
        let result = worksheet()
            .calculate(dec!(433.13), dec!(0.00), dec!(0.00))
            .unwrap();

        assert!(!result.below_threshold);
        assert_eq!(result.net_earnings, dec!(400.00));
        assert_eq!(result.medicare_tax, dec!(11.60));
        assert_eq!(result.social_security_tax, dec!(49.60));
        assert_eq!(result.self_employment_tax, dec!(61.20));
        assert_eq!(result.se_tax_deduction, dec!(30.60));
    }

    #[test]
    fn calculate_skips_tax_for_negative_se_income() {
        let result = worksheet()
            .calculate(dec!(-10000.00), dec!(0.00), dec!(0.00))
            .unwrap();

        assert!(result.below_threshold);
        assert_eq!(result.combined_se_income, dec!(-10000.00));
        assert_eq!(result.self_employment_tax, dec!(0.00));
    }

    #[test]
    fn calculate_skips_tax_when_crp_payments_exceed_se_income() {
        let result = worksheet()
            .calculate(dec!(1000.00), dec!(2000.00), dec!(0.00))
            .unwrap();

        // Line 2: 1000 − 2000 = −1000 (a subtraction in the other direction would be positive)
        assert!(result.below_threshold);
        assert_eq!(result.combined_se_income, dec!(-1000.00));
        assert_eq!(result.self_employment_tax, dec!(0.00));
    }

    #[test]
    fn calculate_returns_error_for_invalid_config() {
        let config = SeWorksheetConfig {
            ss_wage_max: dec!(-1000.00),
            ..test_config()
        };

        let result = SeWorksheet::new(config).calculate(dec!(100000.00), dec!(0.00), dec!(0.00));

        assert_eq!(
            result,
            Err(SeWorksheetError::InvalidSsWageMax(dec!(-1000.00)))
        );
    }
}
