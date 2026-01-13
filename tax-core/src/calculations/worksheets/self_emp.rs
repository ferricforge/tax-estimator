//! Self-Employment Tax Worksheet calculations for IRS Form 1040-ES.
//!
//! This module implements the SE Tax and Deduction Worksheet from Form 1040-ES,
//! which calculates self-employment tax and the deductible portion of that tax.
//!
//! # Worksheet Structure
//!
//! The SE worksheet consists of the following lines:
//!
//! | Line | Description |
//! |------|-------------|
//! | 1a   | Net farm profit or loss from Schedule F, line 34 |
//! | 1b   | Conservation Reserve Program payments (if applicable) |
//! | 2    | Net profit or loss from self-employment (Schedule C, etc.) |
//! | 3    | Combined net earnings × 92.35% (net earnings factor) |
//! | 4    | Medicare tax: Line 3 × 2.9% |
//! | 5    | Maximum earnings subject to social security tax |
//! | 6    | Total wages and tips subject to social security tax |
//! | 7    | Line 5 minus Line 6 (if zero or less, skip to Line 10) |
//! | 8    | Smaller of Line 3 or Line 7 |
//! | 9    | Social security tax: Line 8 × 12.4% |
//! | 10   | Self-employment tax: Line 4 + Line 9 |
//! | 11   | Deductible part of SE tax: Line 10 × 50% |
//!
//! # Minimum Threshold
//!
//! If net earnings from self-employment are $400 or less, no self-employment
//! tax is due and Schedule SE is not required. This threshold is configurable
//! via [`SeWorksheetConfig::min_se_threshold`].
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

    /// Minimum net earnings threshold for SE tax to apply.
    ///
    /// If net earnings from self-employment are at or below this amount,
    /// no SE tax is due. For 2025, this is $400.
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
    ///     se_tax_deductible_percentage: dec!(0.9235),
    ///     se_deduction_factor: dec!(0.50),
    ///     required_payment_threshold: dec!(1000.00),
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
            net_earnings_factor: config.se_tax_deductible_percentage,
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
        if self.ss_tax_rate < Decimal::ZERO || self.ss_tax_rate > Decimal::ONE {
            return Err(SeWorksheetError::InvalidSocialSecurityRate(
                self.ss_tax_rate,
            ));
        }
        if self.medicare_tax_rate < Decimal::ZERO || self.medicare_tax_rate > Decimal::ONE {
            return Err(SeWorksheetError::InvalidMedicareRate(
                self.medicare_tax_rate,
            ));
        }
        if self.deduction_factor < Decimal::ZERO || self.deduction_factor > Decimal::ONE {
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

/// Result of SE worksheet calculations.
///
/// Contains both the self-employment tax amount and the deductible portion
/// of that tax, along with intermediate calculation values for transparency.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeWorksheetResult {
    /// Combined self-employment income before applying the net earnings factor.
    ///
    /// This is the sum of SE income and CRP payments (Lines 1a + 1b + 2).
    pub combined_se_income: Decimal,

    /// Net earnings from self-employment after applying the net earnings factor (Line 3).
    ///
    /// This is the combined SE income × 92.35% (or configured factor).
    pub net_earnings: Decimal,

    /// Medicare tax component (Line 4).
    ///
    /// Calculated as net_earnings × 2.9% (or configured rate).
    pub medicare_tax: Decimal,

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

    /// Indicates whether SE tax was skipped due to income below threshold.
    ///
    /// If `true`, the combined SE income was at or below the minimum threshold
    /// (typically $400), so no SE tax is due.
    pub below_threshold: bool,
}

impl SeWorksheetResult {
    /// Creates a zero-valued result for income below the SE threshold.
    fn below_threshold(combined_se_income: Decimal) -> Self {
        Self {
            combined_se_income,
            net_earnings: Decimal::ZERO,
            medicare_tax: Decimal::ZERO,
            ss_taxable_earnings: Decimal::ZERO,
            social_security_tax: Decimal::ZERO,
            self_employment_tax: Decimal::ZERO,
            se_tax_deduction: Decimal::ZERO,
            below_threshold: true,
        }
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
    /// * `se_income` - Combined self-employment income (Lines 1a + 2)
    /// * `crp_payments` - Conservation Reserve Program payments (Line 1b)
    /// * `wages` - Total wages subject to social security tax (Line 6)
    ///
    /// # Returns
    ///
    /// Returns [`SeWorksheetResult`] containing the calculated SE tax and deduction,
    /// along with intermediate values. If combined SE income is at or below the
    /// minimum threshold ($400 for 2025), returns a zero-valued result with
    /// `below_threshold` set to `true`.
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
    /// // SE income at or below $400 threshold
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

        // Lines 1a, 1b, 2: Combined into se_income + crp_payments
        let combined_income = self.combined_se_income(se_income, crp_payments);

        // Check minimum threshold - if at or below $400, no SE tax is due
        if combined_income <= self.config.min_se_threshold {
            warn!(
                combined_income = %combined_income,
                threshold = %self.config.min_se_threshold,
                "SE income at or below minimum threshold; no SE tax due"
            );
            return Ok(SeWorksheetResult::below_threshold(combined_income));
        }

        // Line 3: Net earnings from self-employment
        let net_earnings = self.net_earnings_from_self_employment(combined_income);

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
            combined_se_income: combined_income,
            net_earnings,
            medicare_tax,
            ss_taxable_earnings,
            social_security_tax,
            self_employment_tax,
            se_tax_deduction,
            below_threshold: false,
        })
    }

    /// Combines self-employment income sources (Lines 1a, 1b, 2).
    ///
    /// Adds the net self-employment income and any Conservation Reserve Program
    /// (CRP) payments to get the total income subject to SE tax.
    ///
    /// # Form Reference
    ///
    /// - Line 1a: Net farm profit or loss from Schedule F
    /// - Line 1b: Conservation Reserve Program payments
    /// - Line 2: Net profit from non-farm self-employment
    fn combined_se_income(
        &self,
        se_income: Decimal,
        crp_payments: Decimal,
    ) -> Decimal {
        let combined = se_income + crp_payments;
        if combined < Decimal::ZERO {
            warn!(
                se_income = %se_income,
                crp_payments = %crp_payments,
                combined = %combined,
                "Combined SE income is negative; SE tax will be zero"
            );
        }
        round_half_up(combined)
    }

    /// Calculates net earnings from self-employment (Line 3).
    ///
    /// Multiplies the combined SE income by the net earnings factor (typically 92.35%)
    /// to arrive at the amount subject to SE tax calculation.
    ///
    /// # Form Reference
    ///
    /// Line 3: Multiply lines 1a, 1b, and 2 combined by 92.35% (0.9235)
    fn net_earnings_from_self_employment(
        &self,
        combined_income: Decimal,
    ) -> Decimal {
        let net_earnings = combined_income * self.config.net_earnings_factor;
        let rounded = round_half_up(net_earnings);

        // If negative, SE tax is zero but we return the value for transparency
        if rounded < Decimal::ZERO {
            warn!(
                combined_income = %combined_income,
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
    /// wage base. If wages exceed the maximum, returns zero.
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
    use tracing_subscriber::fmt::format::FmtSpan;

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

    /// Initializes tracing subscriber for tests that verify log output.
    fn init_test_tracing() -> tracing::subscriber::DefaultGuard {
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::WARN)
            .with_span_events(FmtSpan::NONE)
            .with_test_writer()
            .finish();
        tracing::subscriber::set_default(subscriber)
    }

    // =========================================================================
    // SeWorksheetConfig::validate tests
    // =========================================================================

    #[test]
    fn validate_accepts_valid_config() {
        let config = test_config();

        let result = config.validate();

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
    fn validate_rejects_negative_net_earnings_factor() {
        let config = SeWorksheetConfig {
            net_earnings_factor: dec!(-0.5),
            ..test_config()
        };

        let result = config.validate();

        assert_eq!(
            result,
            Err(SeWorksheetError::InvalidNetEarningsFactor(dec!(-0.5)))
        );
    }

    #[test]
    fn validate_rejects_net_earnings_factor_greater_than_one() {
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
    fn validate_rejects_ss_tax_rate_greater_than_one() {
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
    fn validate_rejects_medicare_rate_greater_than_one() {
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
    fn validate_rejects_deduction_factor_greater_than_one() {
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
    fn validate_rejects_negative_ss_wage_max() {
        let config = SeWorksheetConfig {
            ss_wage_max: dec!(-1000.00),
            ..test_config()
        };

        let result = config.validate();

        assert_eq!(
            result,
            Err(SeWorksheetError::InvalidSsWageMax(dec!(-1000.00)))
        );
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
    // SeWorksheetConfig::from_tax_year_config tests
    // =========================================================================

    #[test]
    fn from_tax_year_config_maps_all_fields() {
        let tax_year_config = TaxYearConfig {
            tax_year: 2025,
            ss_wage_max: dec!(176100.00),
            ss_tax_rate: dec!(0.124),
            medicare_tax_rate: dec!(0.029),
            se_tax_deductible_percentage: dec!(0.9235),
            se_deduction_factor: dec!(0.50),
            required_payment_threshold: dec!(1000.00),
            min_se_threshold: dec!(400.00),
        };

        let config = SeWorksheetConfig::from_tax_year_config(&tax_year_config);

        assert_eq!(config.ss_wage_max, dec!(176100.00));
        assert_eq!(config.ss_tax_rate, dec!(0.124));
        assert_eq!(config.medicare_tax_rate, dec!(0.029));
        assert_eq!(config.net_earnings_factor, dec!(0.9235));
        assert_eq!(config.deduction_factor, dec!(0.50));
        assert_eq!(config.min_se_threshold, dec!(400.00));
    }

    #[test]
    fn from_tax_year_config_handles_different_year_values() {
        let tax_year_config = TaxYearConfig {
            tax_year: 2024,
            ss_wage_max: dec!(168600.00),
            ss_tax_rate: dec!(0.124),
            medicare_tax_rate: dec!(0.029),
            se_tax_deductible_percentage: dec!(0.9235),
            se_deduction_factor: dec!(0.50),
            required_payment_threshold: dec!(1000.00),
            min_se_threshold: dec!(400.00),
        };

        let config = SeWorksheetConfig::from_tax_year_config(&tax_year_config);

        assert_eq!(config.ss_wage_max, dec!(168600.00));
    }

    #[test]
    fn from_tax_year_config_uses_configured_threshold() {
        let tax_year_config = TaxYearConfig {
            tax_year: 2026,
            ss_wage_max: dec!(180000.00),
            ss_tax_rate: dec!(0.124),
            medicare_tax_rate: dec!(0.029),
            se_tax_deductible_percentage: dec!(0.9235),
            se_deduction_factor: dec!(0.50),
            required_payment_threshold: dec!(1000.00),
            min_se_threshold: dec!(450.00), // Different threshold for 2026
        };

        let config = SeWorksheetConfig::from_tax_year_config(&tax_year_config);

        assert_eq!(config.min_se_threshold, dec!(450.00));
    }

    // =========================================================================
    // combined_se_income tests (Lines 1a, 1b, 2)
    // =========================================================================

    #[test]
    fn combined_se_income_adds_se_income_and_crp_payments() {
        let worksheet = SeWorksheet::new(test_config());

        let result = worksheet.combined_se_income(dec!(50000.00), dec!(5000.00));

        assert_eq!(result, dec!(55000.00));
    }

    #[test]
    fn combined_se_income_handles_zero_crp_payments() {
        let worksheet = SeWorksheet::new(test_config());

        let result = worksheet.combined_se_income(dec!(75000.00), dec!(0.00));

        assert_eq!(result, dec!(75000.00));
    }

    #[test]
    fn combined_se_income_handles_zero_se_income() {
        let worksheet = SeWorksheet::new(test_config());

        let result = worksheet.combined_se_income(dec!(0.00), dec!(3000.00));

        assert_eq!(result, dec!(3000.00));
    }

    #[test]
    fn combined_se_income_logs_warning_for_negative_result() {
        let _guard = init_test_tracing();
        let worksheet = SeWorksheet::new(test_config());

        let result = worksheet.combined_se_income(dec!(-10000.00), dec!(5000.00));

        assert_eq!(result, dec!(-5000.00));
        // Warning is logged (verified by test_writer capturing output)
    }

    #[test]
    fn combined_se_income_rounds_to_two_decimal_places() {
        let worksheet = SeWorksheet::new(test_config());

        let result = worksheet.combined_se_income(dec!(100.126), dec!(200.127));

        assert_eq!(result, dec!(300.25)); // 300.253 rounds to 300.25
    }

    // =========================================================================
    // net_earnings_from_self_employment tests (Line 3)
    // =========================================================================

    #[test]
    fn net_earnings_applies_factor_to_combined_income() {
        let worksheet = SeWorksheet::new(test_config());

        let result = worksheet.net_earnings_from_self_employment(dec!(100000.00));

        assert_eq!(result, dec!(92350.00)); // 100000 × 0.9235
    }

    #[test]
    fn net_earnings_handles_zero_income() {
        let worksheet = SeWorksheet::new(test_config());

        let result = worksheet.net_earnings_from_self_employment(dec!(0.00));

        assert_eq!(result, dec!(0.00));
    }

    #[test]
    fn net_earnings_logs_warning_for_negative_result() {
        let _guard = init_test_tracing();
        let worksheet = SeWorksheet::new(test_config());

        let result = worksheet.net_earnings_from_self_employment(dec!(-10000.00));

        assert_eq!(result, dec!(-9235.00)); // -10000 × 0.9235
        // Warning is logged
    }

    #[test]
    fn net_earnings_rounds_half_up() {
        let worksheet = SeWorksheet::new(test_config());

        // 12345.67 × 0.9235 = 11401.226245, rounds to 11401.23
        let result = worksheet.net_earnings_from_self_employment(dec!(12345.67));

        assert_eq!(result, dec!(11401.23));
    }

    // =========================================================================
    // medicare_tax tests (Line 4)
    // =========================================================================

    #[test]
    fn medicare_tax_applies_rate_to_net_earnings() {
        let worksheet = SeWorksheet::new(test_config());

        let result = worksheet.medicare_tax(dec!(92350.00));

        assert_eq!(result, dec!(2678.15)); // 92350 × 0.029
    }

    #[test]
    fn medicare_tax_returns_zero_for_zero_earnings() {
        let _guard = init_test_tracing();
        let worksheet = SeWorksheet::new(test_config());

        let result = worksheet.medicare_tax(dec!(0.00));

        assert_eq!(result, dec!(0.00));
        // Warning is logged
    }

    #[test]
    fn medicare_tax_returns_zero_for_negative_earnings() {
        let _guard = init_test_tracing();
        let worksheet = SeWorksheet::new(test_config());

        let result = worksheet.medicare_tax(dec!(-5000.00));

        assert_eq!(result, dec!(0.00));
        // Warning is logged
    }

    #[test]
    fn medicare_tax_applies_to_all_net_earnings_without_cap() {
        let worksheet = SeWorksheet::new(test_config());

        // Unlike SS, Medicare has no wage cap
        // 500000 × 0.029 = 14500
        let result = worksheet.medicare_tax(dec!(500000.00));

        assert_eq!(result, dec!(14500.00));
    }

    #[test]
    fn medicare_tax_rounds_half_up() {
        let worksheet = SeWorksheet::new(test_config());

        // 10000.55 × 0.029 = 290.01595, rounds to 290.02
        let result = worksheet.medicare_tax(dec!(10000.55));

        assert_eq!(result, dec!(290.02));
    }

    // =========================================================================
    // remaining_ss_wage_base tests (Line 7)
    // =========================================================================

    #[test]
    fn remaining_ss_wage_base_subtracts_wages_from_maximum() {
        let worksheet = SeWorksheet::new(test_config());

        let result = worksheet.remaining_ss_wage_base(dec!(50000.00));

        assert_eq!(result, dec!(126100.00)); // 176100 - 50000
    }

    #[test]
    fn remaining_ss_wage_base_returns_zero_when_wages_exceed_max() {
        let _guard = init_test_tracing();
        let worksheet = SeWorksheet::new(test_config());

        let result = worksheet.remaining_ss_wage_base(dec!(200000.00));

        assert_eq!(result, dec!(0.00));
        // Warning is logged
    }

    #[test]
    fn remaining_ss_wage_base_returns_zero_when_wages_equal_max() {
        let _guard = init_test_tracing();
        let worksheet = SeWorksheet::new(test_config());

        let result = worksheet.remaining_ss_wage_base(dec!(176100.00));

        assert_eq!(result, dec!(0.00));
        // Warning is logged
    }

    #[test]
    fn remaining_ss_wage_base_handles_zero_wages() {
        let worksheet = SeWorksheet::new(test_config());

        let result = worksheet.remaining_ss_wage_base(dec!(0.00));

        assert_eq!(result, dec!(176100.00));
    }

    #[test]
    fn remaining_ss_wage_base_rounds_to_two_decimal_places() {
        let config = SeWorksheetConfig {
            ss_wage_max: dec!(176100.127),
            ..test_config()
        };
        let worksheet = SeWorksheet::new(config);

        let result = worksheet.remaining_ss_wage_base(dec!(100.004));

        // 176100.127 - 100.004 = 176000.123, rounds to 176000.12
        assert_eq!(result, dec!(176000.12));
    }

    // =========================================================================
    // ss_taxable_earnings tests (Line 8)
    // =========================================================================

    #[test]
    fn ss_taxable_earnings_returns_net_earnings_when_less_than_base() {
        let worksheet = SeWorksheet::new(test_config());

        let result = worksheet.ss_taxable_earnings(dec!(50000.00), dec!(100000.00));

        assert_eq!(result, dec!(50000.00));
    }

    #[test]
    fn ss_taxable_earnings_returns_base_when_less_than_net_earnings() {
        let worksheet = SeWorksheet::new(test_config());

        let result = worksheet.ss_taxable_earnings(dec!(100000.00), dec!(50000.00));

        assert_eq!(result, dec!(50000.00));
    }

    #[test]
    fn ss_taxable_earnings_returns_zero_for_negative_net_earnings() {
        let _guard = init_test_tracing();
        let worksheet = SeWorksheet::new(test_config());

        let result = worksheet.ss_taxable_earnings(dec!(-5000.00), dec!(100000.00));

        assert_eq!(result, dec!(0.00));
        // Warning is logged
    }

    #[test]
    fn ss_taxable_earnings_returns_zero_for_zero_net_earnings() {
        let _guard = init_test_tracing();
        let worksheet = SeWorksheet::new(test_config());

        let result = worksheet.ss_taxable_earnings(dec!(0.00), dec!(100000.00));

        assert_eq!(result, dec!(0.00));
        // Warning is logged
    }

    #[test]
    fn ss_taxable_earnings_handles_equal_values() {
        let worksheet = SeWorksheet::new(test_config());

        let result = worksheet.ss_taxable_earnings(dec!(75000.00), dec!(75000.00));

        assert_eq!(result, dec!(75000.00));
    }

    // =========================================================================
    // social_security_tax tests (Line 9)
    // =========================================================================

    #[test]
    fn social_security_tax_applies_rate_to_taxable_earnings() {
        let worksheet = SeWorksheet::new(test_config());

        let result = worksheet.social_security_tax(dec!(92350.00));

        assert_eq!(result, dec!(11451.40)); // 92350 × 0.124
    }

    #[test]
    fn social_security_tax_handles_zero_earnings() {
        let worksheet = SeWorksheet::new(test_config());

        let result = worksheet.social_security_tax(dec!(0.00));

        assert_eq!(result, dec!(0.00));
    }

    #[test]
    fn social_security_tax_rounds_half_up() {
        let worksheet = SeWorksheet::new(test_config());

        // 10000.55 × 0.124 = 1240.0682, rounds to 1240.07
        let result = worksheet.social_security_tax(dec!(10000.55));

        assert_eq!(result, dec!(1240.07));
    }

    #[test]
    fn social_security_tax_handles_large_earnings() {
        let worksheet = SeWorksheet::new(test_config());

        // Max SS earnings of 176100 × 0.124 = 21836.40
        let result = worksheet.social_security_tax(dec!(176100.00));

        assert_eq!(result, dec!(21836.40));
    }

    // =========================================================================
    // total_self_employment_tax tests (Line 10)
    // =========================================================================

    #[test]
    fn total_se_tax_adds_medicare_and_ss_taxes() {
        let worksheet = SeWorksheet::new(test_config());

        let result = worksheet.total_self_employment_tax(dec!(2678.15), dec!(11451.40));

        assert_eq!(result, dec!(14129.55));
    }

    #[test]
    fn total_se_tax_handles_zero_components() {
        let worksheet = SeWorksheet::new(test_config());

        let result = worksheet.total_self_employment_tax(dec!(0.00), dec!(0.00));

        assert_eq!(result, dec!(0.00));
    }

    #[test]
    fn total_se_tax_handles_zero_ss_tax() {
        let worksheet = SeWorksheet::new(test_config());

        let result = worksheet.total_self_employment_tax(dec!(2678.15), dec!(0.00));

        assert_eq!(result, dec!(2678.15));
    }

    #[test]
    fn total_se_tax_rounds_result() {
        let worksheet = SeWorksheet::new(test_config());

        let result = worksheet.total_self_employment_tax(dec!(100.126), dec!(200.127));

        assert_eq!(result, dec!(300.25)); // 300.253 rounds to 300.25
    }

    // =========================================================================
    // se_tax_deduction tests (Line 11)
    // =========================================================================

    #[test]
    fn se_tax_deduction_applies_deduction_factor() {
        let worksheet = SeWorksheet::new(test_config());

        let result = worksheet.se_tax_deduction(dec!(14129.55));

        assert_eq!(result, dec!(7064.78)); // 14129.55 × 0.50, rounded
    }

    #[test]
    fn se_tax_deduction_handles_zero_tax() {
        let worksheet = SeWorksheet::new(test_config());

        let result = worksheet.se_tax_deduction(dec!(0.00));

        assert_eq!(result, dec!(0.00));
    }

    #[test]
    fn se_tax_deduction_rounds_half_up() {
        let worksheet = SeWorksheet::new(test_config());

        // 12345.67 × 0.50 = 6172.835, rounds to 6172.84
        let result = worksheet.se_tax_deduction(dec!(12345.67));

        assert_eq!(result, dec!(6172.84));
    }

    #[test]
    fn se_tax_deduction_handles_odd_pennies() {
        let worksheet = SeWorksheet::new(test_config());

        // 14129.55 × 0.50 = 7064.775, rounds to 7064.78
        let result = worksheet.se_tax_deduction(dec!(14129.55));

        assert_eq!(result, dec!(7064.78));
    }

    // =========================================================================
    // calculate (integration) tests
    // =========================================================================

    #[test]
    fn calculate_returns_correct_result_for_standard_case() {
        let worksheet = SeWorksheet::new(test_config());

        let result = worksheet
            .calculate(dec!(100000.00), dec!(0.00), dec!(50000.00))
            .unwrap();

        assert!(!result.below_threshold);
        assert_eq!(result.combined_se_income, dec!(100000.00));
        // Net earnings: 100000 × 0.9235 = 92350
        assert_eq!(result.net_earnings, dec!(92350.00));
        // Medicare: 92350 × 0.029 = 2678.15
        assert_eq!(result.medicare_tax, dec!(2678.15));
        // Remaining SS base: 176100 - 50000 = 126100
        // SS taxable: min(92350, 126100) = 92350
        assert_eq!(result.ss_taxable_earnings, dec!(92350.00));
        // SS tax: 92350 × 0.124 = 11451.40
        assert_eq!(result.social_security_tax, dec!(11451.40));
        // Total SE tax: 2678.15 + 11451.40 = 14129.55
        assert_eq!(result.self_employment_tax, dec!(14129.55));
        // Deduction: 14129.55 × 0.50 = 7064.78
        assert_eq!(result.se_tax_deduction, dec!(7064.78));
    }

    #[test]
    fn calculate_returns_error_for_invalid_config() {
        let config = SeWorksheetConfig {
            ss_wage_max: dec!(-1000.00),
            ..test_config()
        };
        let worksheet = SeWorksheet::new(config);

        let result = worksheet.calculate(dec!(100000.00), dec!(0.00), dec!(0.00));

        assert_eq!(
            result,
            Err(SeWorksheetError::InvalidSsWageMax(dec!(-1000.00)))
        );
    }

    #[test]
    fn calculate_handles_wages_exceeding_ss_max() {
        let _guard = init_test_tracing();
        let worksheet = SeWorksheet::new(test_config());

        let result = worksheet
            .calculate(dec!(50000.00), dec!(0.00), dec!(200000.00))
            .unwrap();

        assert!(!result.below_threshold);
        // Net earnings: 50000 × 0.9235 = 46175
        assert_eq!(result.net_earnings, dec!(46175.00));
        // Medicare: 46175 × 0.029 = 1339.08 (rounded)
        assert_eq!(result.medicare_tax, dec!(1339.08));
        // SS taxable: 0 (wages exceed max)
        assert_eq!(result.ss_taxable_earnings, dec!(0.00));
        // SS tax: 0
        assert_eq!(result.social_security_tax, dec!(0.00));
        // Total: 1339.08 + 0 = 1339.08
        assert_eq!(result.self_employment_tax, dec!(1339.08));
        // Deduction: 1339.08 × 0.50 = 669.54
        assert_eq!(result.se_tax_deduction, dec!(669.54));
    }

    #[test]
    fn calculate_returns_below_threshold_for_zero_se_income() {
        let _guard = init_test_tracing();
        let worksheet = SeWorksheet::new(test_config());

        let result = worksheet
            .calculate(dec!(0.00), dec!(0.00), dec!(50000.00))
            .unwrap();

        assert!(result.below_threshold);
        assert_eq!(result.combined_se_income, dec!(0.00));
        assert_eq!(result.net_earnings, dec!(0.00));
        assert_eq!(result.medicare_tax, dec!(0.00));
        assert_eq!(result.ss_taxable_earnings, dec!(0.00));
        assert_eq!(result.social_security_tax, dec!(0.00));
        assert_eq!(result.self_employment_tax, dec!(0.00));
        assert_eq!(result.se_tax_deduction, dec!(0.00));
    }

    #[test]
    fn calculate_returns_below_threshold_at_exactly_400() {
        let _guard = init_test_tracing();
        let worksheet = SeWorksheet::new(test_config());

        let result = worksheet
            .calculate(dec!(400.00), dec!(0.00), dec!(0.00))
            .unwrap();

        assert!(result.below_threshold);
        assert_eq!(result.combined_se_income, dec!(400.00));
        assert_eq!(result.self_employment_tax, dec!(0.00));
    }

    #[test]
    fn calculate_computes_tax_just_above_threshold() {
        let worksheet = SeWorksheet::new(test_config());

        let result = worksheet
            .calculate(dec!(400.01), dec!(0.00), dec!(0.00))
            .unwrap();

        assert!(!result.below_threshold);
        assert_eq!(result.combined_se_income, dec!(400.01));
        // Net earnings: 400.01 × 0.9235 = 369.409235, rounds to 369.41
        assert_eq!(result.net_earnings, dec!(369.41));
        // Medicare: 369.41 × 0.029 = 10.71289, rounds to 10.71
        assert_eq!(result.medicare_tax, dec!(10.71));
        // SS taxable: min(369.41, 176100) = 369.41
        assert_eq!(result.ss_taxable_earnings, dec!(369.41));
        // SS tax: 369.41 × 0.124 = 45.80684, rounds to 45.81
        assert_eq!(result.social_security_tax, dec!(45.81));
    }

    #[test]
    fn calculate_includes_crp_payments_in_threshold_check() {
        let _guard = init_test_tracing();
        let worksheet = SeWorksheet::new(test_config());

        // SE income alone is below threshold, but with CRP it's above
        let result = worksheet
            .calculate(dec!(300.00), dec!(200.00), dec!(0.00))
            .unwrap();

        assert!(!result.below_threshold);
        assert_eq!(result.combined_se_income, dec!(500.00));
    }

    #[test]
    fn calculate_returns_below_threshold_for_combined_at_threshold() {
        let _guard = init_test_tracing();
        let worksheet = SeWorksheet::new(test_config());

        // Combined is exactly at threshold
        let result = worksheet
            .calculate(dec!(200.00), dec!(200.00), dec!(0.00))
            .unwrap();

        assert!(result.below_threshold);
        assert_eq!(result.combined_se_income, dec!(400.00));
    }

    #[test]
    fn calculate_includes_crp_payments() {
        let worksheet = SeWorksheet::new(test_config());

        let result = worksheet
            .calculate(dec!(45000.00), dec!(5000.00), dec!(0.00))
            .unwrap();

        // Combined: 45000 + 5000 = 50000
        assert_eq!(result.combined_se_income, dec!(50000.00));
        // Net earnings: 50000 × 0.9235 = 46175
        assert_eq!(result.net_earnings, dec!(46175.00));
    }

    #[test]
    fn calculate_caps_ss_tax_at_wage_base() {
        let worksheet = SeWorksheet::new(test_config());

        // High SE income that would exceed SS wage base
        let result = worksheet
            .calculate(dec!(250000.00), dec!(0.00), dec!(0.00))
            .unwrap();

        // Net earnings: 250000 × 0.9235 = 230875
        assert_eq!(result.net_earnings, dec!(230875.00));
        // Medicare: 230875 × 0.029 = 6695.38 (rounded)
        assert_eq!(result.medicare_tax, dec!(6695.38));
        // SS taxable: min(230875, 176100) = 176100
        assert_eq!(result.ss_taxable_earnings, dec!(176100.00));
        // SS tax: 176100 × 0.124 = 21836.40
        assert_eq!(result.social_security_tax, dec!(21836.40));
    }

    #[test]
    fn calculate_handles_negative_se_income() {
        let _guard = init_test_tracing();
        let worksheet = SeWorksheet::new(test_config());

        let result = worksheet
            .calculate(dec!(-10000.00), dec!(0.00), dec!(0.00))
            .unwrap();

        // Negative income is below threshold
        assert!(result.below_threshold);
        assert_eq!(result.combined_se_income, dec!(-10000.00));
        assert_eq!(result.self_employment_tax, dec!(0.00));
    }
}
