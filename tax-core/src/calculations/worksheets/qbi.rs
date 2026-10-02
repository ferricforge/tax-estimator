//! Qualified Business Income Deduction calculations for IRS Form 8995.
//!
//! This module implements Form 8995, "Qualified Business Income Deduction
//! Simplified Computation". The deduction is up to 20% of net qualified
//! business income (QBI) from a trade or business, plus 20% of qualified real
//! estate investment trust (REIT) dividends and qualified publicly traded
//! partnership (PTP) income. The total deduction is limited to 20% of taxable
//! income, calculated before the QBI deduction, minus net capital gain
//! (increased by any qualified dividends).
//!
//! # Form Structure
//!
//! | Line | Description |
//! |------|-------------|
//! | 1    | Trade, business, or aggregation name, taxpayer identification number, and QBI or (loss) |
//! | 2    | Total qualified business income or (loss): combine lines 1i through 1v, column (c) |
//! | 3    | Qualified business net (loss) carryforward from the prior year |
//! | 4    | Total qualified business income: combine lines 2 and 3 (if zero or less, enter -0-) |
//! | 5    | Qualified business income component: Line 4 × 20% |
//! | 6    | Qualified REIT dividends and PTP income or (loss) |
//! | 7    | Qualified REIT dividends and qualified PTP (loss) carryforward from the prior year |
//! | 8    | Total qualified REIT dividends and PTP income: combine lines 6 and 7 (if zero or less, enter -0-) |
//! | 9    | REIT and PTP component: Line 8 × 20% |
//! | 10   | QBI deduction before the income limitation: Line 5 + Line 9 |
//! | 11   | Taxable income before qualified business income deduction |
//! | 12   | Net capital gain, increased by any qualified dividends |
//! | 13   | Line 11 minus Line 12 (if zero or less, enter -0-) |
//! | 14   | Income limitation: Line 13 × 20% |
//! | 15   | Qualified business income deduction: smaller of Line 10 or Line 14 |
//! | 16   | Total qualified business (loss) carryforward: combine lines 2 and 3 (if greater than zero, enter -0-) |
//! | 17   | Total qualified REIT dividends and PTP (loss) carryforward: combine lines 6 and 7 (if greater than zero, enter -0-) |
//!
//! # Sign Convention
//!
//! Income is a positive number and a loss is a negative number. This includes
//! the loss carryforwards on lines 3 and 7, which the printed form shows in
//! parentheses. The carryforwards on lines 16 and 17 are returned as zero or
//! as a negative number.
//!
//! # Taxable Income Threshold
//!
//! Form 8995 can be used only when taxable income before the QBI deduction is
//! at or below the threshold for the filing status. Above the threshold,
//! Form 8995-A must be used instead. The worksheet still computes every line
//! and reports the condition in [`QbiWorksheetResult::above_threshold`]. The
//! threshold is configured via
//! [`QbiWorksheetConfig::taxable_income_threshold`].
//!
//! # Example
//!
//! ```
//! use rust_decimal_macros::dec;
//! use tax_core::calculations::{QbiWorksheet, QbiWorksheetConfig, QbiWorksheetInput};
//!
//! let config = QbiWorksheetConfig {
//!     deduction_rate: dec!(0.20),
//!     taxable_income_threshold: Some(dec!(197300.00)),
//! };
//!
//! let input = QbiWorksheetInput {
//!     trade_or_business_income: vec![dec!(70000.50)],
//!     taxable_income_before_qbi: Some(dec!(80000.25)),
//!     net_capital_gain: dec!(550.75),
//!     ..QbiWorksheetInput::default()
//! };
//!
//! let worksheet = QbiWorksheet::new(config);
//! let result = worksheet.calculate(&input).unwrap();
//!
//! assert_eq!(result.qbi_component, dec!(14000.10));
//! assert_eq!(result.income_limitation, Some(dec!(15889.90)));
//! assert_eq!(result.qbi_deduction, dec!(14000.10));
//! assert!(!result.above_threshold);
//! ```

use std::fmt;

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::calculations::common::round_half_up;

/// Rate applied to total qualified business income (Line 5), total qualified
/// REIT dividends and PTP income (Line 9), and the income limitation
/// (Line 14). The form specifies 20% (0.20).
pub const QBI_DEDUCTION_RATE: Decimal = Decimal::from_parts(20, 0, 0, false, 2);

/// Errors that can occur during QBI worksheet calculations.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum QbiWorksheetError {
    /// The deduction rate must be between 0 and 1.
    #[error("QBI deduction rate must be between 0 and 1, got {0}")]
    InvalidDeductionRate(Decimal),
    /// The taxable income threshold must be non-negative.
    #[error("QBI taxable income threshold must be non-negative, got {0}")]
    InvalidTaxableIncomeThreshold(Decimal),
}

/// Configuration parameters for QBI worksheet calculations.
///
/// These values are specified by the IRS and may change from year to year.
///
/// # Example
///
/// ```
/// use rust_decimal_macros::dec;
/// use tax_core::calculations::QbiWorksheetConfig;
///
/// // 2025 tax year configuration for a return that is not married filing jointly
/// let config = QbiWorksheetConfig {
///     deduction_rate: dec!(0.20),
///     taxable_income_threshold: Some(dec!(197300.00)),
/// };
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QbiWorksheetConfig {
    /// Rate applied on Lines 5, 9, and 14. The form specifies 20% (0.20).
    pub deduction_rate: Decimal,
    /// Highest taxable income before the QBI deduction for which Form 8995
    /// can be used.
    ///
    /// For 2025, this is $394,600 if married filing jointly, and $197,300
    /// for all other returns. `None` means no threshold is known for the tax
    /// year, so the worksheet never reports income as above the threshold.
    pub taxable_income_threshold: Option<Decimal>,
}

impl QbiWorksheetConfig {
    /// Creates the configuration for a tax year and filing status, using
    /// [`QBI_DEDUCTION_RATE`] and [`qbi_taxable_income_threshold`].
    ///
    /// # Example
    ///
    /// ```
    /// use rust_decimal_macros::dec;
    /// use tax_core::calculations::QbiWorksheetConfig;
    ///
    /// let config = QbiWorksheetConfig::for_tax_year(2025, true);
    ///
    /// assert_eq!(config.deduction_rate, dec!(0.20));
    /// assert_eq!(config.taxable_income_threshold, Some(dec!(394600)));
    /// ```
    pub fn for_tax_year(
        tax_year: i32,
        is_joint: bool,
    ) -> Self {
        Self {
            deduction_rate: QBI_DEDUCTION_RATE,
            taxable_income_threshold: qbi_taxable_income_threshold(tax_year, is_joint),
        }
    }

    /// Validates the configuration values.
    ///
    /// # Errors
    ///
    /// Returns [`QbiWorksheetError`] if:
    /// - `deduction_rate` is not in [0, 1]
    /// - `taxable_income_threshold` is negative
    ///
    /// # Example
    ///
    /// ```
    /// use rust_decimal_macros::dec;
    /// use tax_core::calculations::{QbiWorksheetConfig, QbiWorksheetError};
    ///
    /// let invalid_config = QbiWorksheetConfig {
    ///     deduction_rate: dec!(1.5),
    ///     taxable_income_threshold: None,
    /// };
    ///
    /// let result = invalid_config.validate();
    /// assert_eq!(result, Err(QbiWorksheetError::InvalidDeductionRate(dec!(1.5))));
    /// ```
    pub fn validate(&self) -> Result<(), QbiWorksheetError> {
        if self.deduction_rate < Decimal::ZERO || self.deduction_rate > Decimal::ONE {
            return Err(QbiWorksheetError::InvalidDeductionRate(self.deduction_rate));
        }

        if let Some(threshold) = self.taxable_income_threshold
            && threshold < Decimal::ZERO
        {
            return Err(QbiWorksheetError::InvalidTaxableIncomeThreshold(threshold));
        }

        Ok(())
    }
}

/// Values entered on Form 8995 that the computed lines are figured from.
///
/// Income is a positive number and a loss is a negative number, including the
/// loss carryforwards on Lines 3 and 7.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct QbiWorksheetInput {
    /// Qualified business income or (loss) for each trade, business, or
    /// aggregation (Line 1, column (c)).
    ///
    /// The net QBI or (loss) for the trade, business, or aggregation reported
    /// in the corresponding row. Does not include losses or deductions
    /// suspended from use in calculating taxable income in the current year.
    pub trade_or_business_income: Vec<Decimal>,
    /// Qualified business net (loss) carryforward from the prior year
    /// (Line 3).
    ///
    /// The qualified portion of trade or business (loss) carryforward allowed
    /// in calculating taxable income in the current year, even if the loss
    /// was from a trade or business that is no longer in existence.
    pub qbi_loss_carryforward: Decimal,
    /// Qualified REIT dividends and publicly traded partnership (PTP) income
    /// or (loss) (Line 6).
    pub reit_ptp_income: Decimal,
    /// Qualified REIT dividends and qualified PTP (loss) carryforward from
    /// the prior year (Line 7).
    ///
    /// The qualified portion of PTP (loss) carryforward allowed in
    /// calculating taxable income in the current year, even if the loss was
    /// from a PTP that is no longer held or is no longer in existence.
    pub reit_ptp_loss_carryforward: Decimal,
    /// Taxable income before the qualified business income deduction
    /// (Line 11).
    ///
    /// `None` when the amount is not known yet. See
    /// [`taxable_income_before_qbi`].
    pub taxable_income_before_qbi: Option<Decimal>,
    /// Net capital gain, if any, increased by any qualified dividends
    /// (Line 12).
    pub net_capital_gain: Decimal,
}

/// Result of QBI worksheet calculations.
///
/// Contains the qualified business income deduction and the loss
/// carryforwards, along with the intermediate lines. `None` means the line
/// cannot be figured because taxable income before the QBI deduction is not
/// known.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QbiWorksheetResult {
    /// Total qualified business income or (loss) (Line 2).
    ///
    /// Lines 1i through 1v, column (c), combined.
    pub total_qbi_or_loss: Decimal,
    /// Total qualified business income (Line 4).
    ///
    /// Lines 2 and 3 combined. Zero when the combined amount is zero or less.
    pub total_qbi: Decimal,
    /// Qualified business income component (Line 5).
    ///
    /// Line 4 × 20% (or configured rate).
    pub qbi_component: Decimal,
    /// Total qualified REIT dividends and PTP income (Line 8).
    ///
    /// Lines 6 and 7 combined. Zero when the combined amount is zero or less.
    pub total_reit_ptp_income: Decimal,
    /// REIT and PTP component (Line 9).
    ///
    /// Line 8 × 20% (or configured rate).
    pub reit_ptp_component: Decimal,
    /// Qualified business income deduction before the income limitation
    /// (Line 10).
    ///
    /// Line 5 + Line 9.
    pub deduction_before_income_limitation: Decimal,
    /// Taxable income before the qualified business income deduction
    /// (Line 11), as supplied in the input.
    pub taxable_income_before_qbi: Option<Decimal>,
    /// Taxable income less net capital gain (Line 13).
    ///
    /// Line 11 − Line 12. Zero when the difference is zero or less.
    pub taxable_income_less_net_capital_gain: Option<Decimal>,
    /// Income limitation (Line 14).
    ///
    /// Line 13 × 20% (or configured rate).
    pub income_limitation: Option<Decimal>,
    /// Qualified business income deduction (Line 15).
    ///
    /// The smaller of Line 10 or Line 14. Zero when Line 14 cannot be
    /// figured. This amount is entered on Form 1040 or 1040-SR, line 13a.
    pub qbi_deduction: Decimal,
    /// Total qualified business (loss) carryforward (Line 16).
    ///
    /// Lines 2 and 3 combined. Zero when the combined amount is greater than
    /// zero; otherwise a negative number. This amount is carried forward to
    /// the next year and offsets QBI in later tax years.
    pub total_qbi_loss_carryforward: Decimal,
    /// Total qualified REIT dividends and PTP (loss) carryforward (Line 17).
    ///
    /// Lines 6 and 7 combined. Zero when the combined amount is greater than
    /// zero; otherwise a negative number. This amount is carried forward to
    /// the next year and offsets qualified REIT dividends and qualified PTP
    /// income in later tax years.
    pub total_reit_ptp_loss_carryforward: Decimal,
    /// Indicates whether taxable income before the QBI deduction is above
    /// the threshold for Form 8995.
    ///
    /// If `true`, Form 8995-A must be used to figure the deduction instead.
    pub above_threshold: bool,
}

impl fmt::Display for QbiWorksheetResult {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        writeln!(f, "QbiWorksheetResult {{")?;
        writeln!(
            f,
            "    total_qbi_or_loss                    : ${}",
            self.total_qbi_or_loss.round_dp(2)
        )?;
        writeln!(
            f,
            "    total_qbi                            : ${}",
            self.total_qbi.round_dp(2)
        )?;
        writeln!(
            f,
            "    qbi_component                        : ${}",
            self.qbi_component.round_dp(2)
        )?;
        writeln!(
            f,
            "    total_reit_ptp_income                : ${}",
            self.total_reit_ptp_income.round_dp(2)
        )?;
        writeln!(
            f,
            "    reit_ptp_component                   : ${}",
            self.reit_ptp_component.round_dp(2)
        )?;
        writeln!(
            f,
            "    deduction_before_income_limitation   : ${}",
            self.deduction_before_income_limitation.round_dp(2)
        )?;
        writeln!(
            f,
            "    taxable_income_before_qbi            : {}",
            optional_amount_display(self.taxable_income_before_qbi)
        )?;
        writeln!(
            f,
            "    taxable_income_less_net_capital_gain : {}",
            optional_amount_display(self.taxable_income_less_net_capital_gain)
        )?;
        writeln!(
            f,
            "    income_limitation                    : {}",
            optional_amount_display(self.income_limitation)
        )?;
        writeln!(
            f,
            "    qbi_deduction                        : ${}",
            self.qbi_deduction.round_dp(2)
        )?;
        writeln!(
            f,
            "    total_qbi_loss_carryforward          : ${}",
            self.total_qbi_loss_carryforward.round_dp(2)
        )?;
        writeln!(
            f,
            "    total_reit_ptp_loss_carryforward     : ${}",
            self.total_reit_ptp_loss_carryforward.round_dp(2)
        )?;
        writeln!(
            f,
            "    above_threshold                      : {}",
            self.above_threshold
        )?;
        write!(f, "}}")?;
        Ok(())
    }
}

/// Calculator for Form 8995, the simplified QBI deduction computation.
///
/// This struct encapsulates the configuration and provides methods to
/// calculate each computed line of the form, ending with the qualified
/// business income deduction and the loss carryforwards.
///
/// # Example
///
/// ```
/// use rust_decimal_macros::dec;
/// use tax_core::calculations::{QbiWorksheet, QbiWorksheetConfig, QbiWorksheetInput};
///
/// let worksheet = QbiWorksheet::new(QbiWorksheetConfig::for_tax_year(2025, false));
///
/// let input = QbiWorksheetInput {
///     trade_or_business_income: vec![dec!(100000.00)],
///     taxable_income_before_qbi: Some(dec!(10000.00)),
///     ..QbiWorksheetInput::default()
/// };
///
/// let result = worksheet.calculate(&input).unwrap();
///
/// // QBI component = $100,000 × 0.20 = $20,000
/// assert_eq!(result.qbi_component, dec!(20000.00));
///
/// // Income limitation = $10,000 × 0.20 = $2,000
/// assert_eq!(result.qbi_deduction, dec!(2000.00));
/// ```
#[derive(Debug, Clone)]
pub struct QbiWorksheet {
    config: QbiWorksheetConfig,
}

impl QbiWorksheet {
    /// Creates a new QBI worksheet calculator with the given configuration.
    pub fn new(config: QbiWorksheetConfig) -> Self {
        Self { config }
    }

    /// Calculates every computed line of Form 8995 and returns the result.
    ///
    /// Validates the configuration, figures Lines 2, 4, 5, 8, 9, 10, 13, 14,
    /// 15, 16, and 17, and checks taxable income against the threshold.
    ///
    /// # Errors
    ///
    /// Returns [`QbiWorksheetError`] if the configuration is invalid.
    ///
    /// # Example
    ///
    /// ```
    /// use rust_decimal_macros::dec;
    /// use tax_core::calculations::{QbiWorksheet, QbiWorksheetConfig, QbiWorksheetInput};
    ///
    /// let worksheet = QbiWorksheet::new(QbiWorksheetConfig::for_tax_year(2025, false));
    ///
    /// // A net loss after the prior year carryforward
    /// let input = QbiWorksheetInput {
    ///     trade_or_business_income: vec![dec!(1000.00)],
    ///     qbi_loss_carryforward: dec!(-3000.00),
    ///     taxable_income_before_qbi: Some(dec!(10000.00)),
    ///     ..QbiWorksheetInput::default()
    /// };
    ///
    /// let result = worksheet.calculate(&input).unwrap();
    ///
    /// assert_eq!(result.total_qbi, dec!(0.00));
    /// assert_eq!(result.qbi_deduction, dec!(0.00));
    /// assert_eq!(result.total_qbi_loss_carryforward, dec!(-2000.00));
    /// ```
    pub fn calculate(
        &self,
        input: &QbiWorksheetInput,
    ) -> Result<QbiWorksheetResult, QbiWorksheetError> {
        self.config.validate()?;

        // Line 2: Total qualified business income or (loss)
        let total_qbi_or_loss = self.total_qbi_or_loss(&input.trade_or_business_income);

        // Line 4: Total qualified business income
        let total_qbi = self.total_qbi(total_qbi_or_loss, input.qbi_loss_carryforward);

        // Line 5: Qualified business income component
        let qbi_component = self.qbi_component(total_qbi);

        // Line 8: Total qualified REIT dividends and PTP income
        let total_reit_ptp_income =
            self.total_reit_ptp_income(input.reit_ptp_income, input.reit_ptp_loss_carryforward);

        // Line 9: REIT and PTP component
        let reit_ptp_component = self.reit_ptp_component(total_reit_ptp_income);

        // Line 10: QBI deduction before the income limitation
        let deduction_before_income_limitation =
            self.deduction_before_income_limitation(qbi_component, reit_ptp_component);

        // Line 13: Line 11 minus Line 12
        let taxable_income_less_net_capital_gain = self.taxable_income_less_net_capital_gain(
            input.taxable_income_before_qbi,
            input.net_capital_gain,
        );

        // Line 14: Income limitation
        let income_limitation = self.income_limitation(taxable_income_less_net_capital_gain);

        // Line 15: Qualified business income deduction
        let qbi_deduction =
            self.qbi_deduction(deduction_before_income_limitation, income_limitation);

        // Line 16: Total qualified business (loss) carryforward
        let total_qbi_loss_carryforward =
            self.total_qbi_loss_carryforward(total_qbi_or_loss, input.qbi_loss_carryforward);

        // Line 17: Total qualified REIT dividends and PTP (loss) carryforward
        let total_reit_ptp_loss_carryforward = self.total_reit_ptp_loss_carryforward(
            input.reit_ptp_income,
            input.reit_ptp_loss_carryforward,
        );

        Ok(QbiWorksheetResult {
            total_qbi_or_loss,
            total_qbi,
            qbi_component,
            total_reit_ptp_income,
            reit_ptp_component,
            deduction_before_income_limitation,
            taxable_income_before_qbi: input.taxable_income_before_qbi,
            taxable_income_less_net_capital_gain,
            income_limitation,
            qbi_deduction,
            total_qbi_loss_carryforward,
            total_reit_ptp_loss_carryforward,
            above_threshold: self.exceeds_threshold(input.taxable_income_before_qbi),
        })
    }

    /// Totals qualified business income or (loss) (Line 2).
    ///
    /// # Form Reference
    ///
    /// Line 2: Total qualified business income or (loss). Combine lines 1i
    /// through 1v, column (c)
    fn total_qbi_or_loss(
        &self,
        trade_or_business_income: &[Decimal],
    ) -> Decimal {
        trade_or_business_income.iter().copied().sum()
    }

    /// Calculates total qualified business income (Line 4).
    ///
    /// If there is a qualified business net loss for the year, there is no
    /// QBI deduction unless there are qualified REIT dividends or qualified
    /// PTP income. The loss is carried forward to the next year.
    ///
    /// # Form Reference
    ///
    /// Line 4: Total qualified business income. Combine lines 2 and 3. If
    /// zero or less, enter -0-
    fn total_qbi(
        &self,
        total_qbi_or_loss: Decimal,
        qbi_loss_carryforward: Decimal,
    ) -> Decimal {
        (total_qbi_or_loss + qbi_loss_carryforward).max(Decimal::ZERO)
    }

    /// Calculates the qualified business income component (Line 5).
    ///
    /// # Form Reference
    ///
    /// Line 5: Qualified business income component. Multiply line 4 by 20%
    /// (0.20)
    fn qbi_component(
        &self,
        total_qbi: Decimal,
    ) -> Decimal {
        round_half_up(total_qbi * self.config.deduction_rate)
    }

    /// Calculates total qualified REIT dividends and PTP income (Line 8).
    ///
    /// Any negative amount is carried forward to the next year.
    ///
    /// # Form Reference
    ///
    /// Line 8: Total qualified REIT dividends and PTP income. Combine lines 6
    /// and 7. If zero or less, enter -0-
    fn total_reit_ptp_income(
        &self,
        reit_ptp_income: Decimal,
        reit_ptp_loss_carryforward: Decimal,
    ) -> Decimal {
        (reit_ptp_income + reit_ptp_loss_carryforward).max(Decimal::ZERO)
    }

    /// Calculates the REIT and PTP component (Line 9).
    ///
    /// # Form Reference
    ///
    /// Line 9: REIT and PTP component. Multiply line 8 by 20% (0.20)
    fn reit_ptp_component(
        &self,
        total_reit_ptp_income: Decimal,
    ) -> Decimal {
        round_half_up(total_reit_ptp_income * self.config.deduction_rate)
    }

    /// Calculates the QBI deduction before the income limitation (Line 10).
    ///
    /// # Form Reference
    ///
    /// Line 10: Qualified business income deduction before the income
    /// limitation. Add lines 5 and 9
    fn deduction_before_income_limitation(
        &self,
        qbi_component: Decimal,
        reit_ptp_component: Decimal,
    ) -> Decimal {
        qbi_component + reit_ptp_component
    }

    /// Subtracts net capital gain from taxable income (Line 13).
    ///
    /// Returns `None` when taxable income before the QBI deduction is not
    /// known.
    ///
    /// # Form Reference
    ///
    /// Line 13: Subtract line 12 from line 11. If zero or less, enter -0-
    fn taxable_income_less_net_capital_gain(
        &self,
        taxable_income_before_qbi: Option<Decimal>,
        net_capital_gain: Decimal,
    ) -> Option<Decimal> {
        taxable_income_before_qbi.map(|income| (income - net_capital_gain).max(Decimal::ZERO))
    }

    /// Calculates the income limitation (Line 14).
    ///
    /// Returns `None` when Line 13 cannot be figured.
    ///
    /// # Form Reference
    ///
    /// Line 14: Income limitation. Multiply line 13 by 20% (0.20)
    fn income_limitation(
        &self,
        taxable_income_less_net_capital_gain: Option<Decimal>,
    ) -> Option<Decimal> {
        taxable_income_less_net_capital_gain
            .map(|amount| round_half_up(amount * self.config.deduction_rate))
    }

    /// Determines the qualified business income deduction (Line 15).
    ///
    /// When the income limitation cannot be figured, the deduction is zero.
    ///
    /// # Form Reference
    ///
    /// Line 15: Qualified business income deduction. Enter the smaller of
    /// line 10 or line 14
    fn qbi_deduction(
        &self,
        deduction_before_income_limitation: Decimal,
        income_limitation: Option<Decimal>,
    ) -> Decimal {
        deduction_before_income_limitation.min(income_limitation.unwrap_or(Decimal::ZERO))
    }

    /// Calculates the total qualified business (loss) carryforward (Line 16).
    ///
    /// # Form Reference
    ///
    /// Line 16: Total qualified business (loss) carryforward. Combine lines 2
    /// and 3. If greater than zero, enter -0-
    fn total_qbi_loss_carryforward(
        &self,
        total_qbi_or_loss: Decimal,
        qbi_loss_carryforward: Decimal,
    ) -> Decimal {
        (total_qbi_or_loss + qbi_loss_carryforward).min(Decimal::ZERO)
    }

    /// Calculates the total qualified REIT dividends and PTP (loss)
    /// carryforward (Line 17).
    ///
    /// # Form Reference
    ///
    /// Line 17: Total qualified REIT dividends and PTP (loss) carryforward.
    /// Combine lines 6 and 7. If greater than zero, enter -0-
    fn total_reit_ptp_loss_carryforward(
        &self,
        reit_ptp_income: Decimal,
        reit_ptp_loss_carryforward: Decimal,
    ) -> Decimal {
        (reit_ptp_income + reit_ptp_loss_carryforward).min(Decimal::ZERO)
    }

    /// Returns `true` when taxable income before the QBI deduction is
    /// strictly above the configured threshold. A missing value on either
    /// side never counts as above.
    ///
    /// # Form Reference
    ///
    /// Use this form if your taxable income, before your qualified business
    /// income deduction, is at or below the threshold for your filing status.
    fn exceeds_threshold(
        &self,
        taxable_income_before_qbi: Option<Decimal>,
    ) -> bool {
        match (
            taxable_income_before_qbi,
            self.config.taxable_income_threshold,
        ) {
            (Some(income), Some(threshold)) => income > threshold,
            _ => false,
        }
    }
}

/// Figures taxable income before the QBI deduction (Form 8995, Line 11).
///
/// For Form 1040 or 1040-SR filers, this is Form 1040 or 1040-SR, line 11a,
/// minus lines 12e and 13b. Line 13b is not included because the application
/// does not collect it yet. A missing deduction counts as zero. Returns
/// `None` only when adjusted gross income is missing.
///
/// # Example
///
/// ```
/// use rust_decimal_macros::dec;
/// use tax_core::calculations::taxable_income_before_qbi;
///
/// assert_eq!(
///     taxable_income_before_qbi(Some(dec!(120000)), Some(dec!(20000))),
///     Some(dec!(100000))
/// );
/// assert_eq!(
///     taxable_income_before_qbi(Some(dec!(120000)), None),
///     Some(dec!(120000))
/// );
/// assert_eq!(taxable_income_before_qbi(None, Some(dec!(20000))), None);
/// ```
pub fn taxable_income_before_qbi(
    expected_agi: Option<Decimal>,
    expected_deduction: Option<Decimal>,
) -> Option<Decimal> {
    expected_agi.map(|agi| agi - expected_deduction.unwrap_or_default())
}

/// Taxable-income threshold for Form 8995, by tax year and filing status.
///
/// Temporary: the 2025 values come from the Form 8995 instructions ($394,600
/// if married filing jointly, and $197,300 for all other returns). These move
/// to tax-year configuration in a later step. `None` means no threshold is
/// known for the year.
///
/// # Example
///
/// ```
/// use rust_decimal_macros::dec;
/// use tax_core::calculations::qbi_taxable_income_threshold;
///
/// assert_eq!(qbi_taxable_income_threshold(2025, false), Some(dec!(197300)));
/// assert_eq!(qbi_taxable_income_threshold(2025, true), Some(dec!(394600)));
/// assert_eq!(qbi_taxable_income_threshold(2030, true), None);
/// ```
pub fn qbi_taxable_income_threshold(
    tax_year: i32,
    is_joint: bool,
) -> Option<Decimal> {
    match (tax_year, is_joint) {
        (2025, true) => Some(Decimal::new(394_600, 0)),
        (2025, false) => Some(Decimal::new(197_300, 0)),
        _ => None,
    }
}

/// Formats an optional amount for display, using "—" when `None`.
fn optional_amount_display(value: Option<Decimal>) -> String {
    value
        .map(|amount| format!("${}", amount.round_dp(2)))
        .unwrap_or_else(|| "—".to_string())
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use rust_decimal_macros::dec;

    use super::*;

    /// Creates the 2025 configuration for a return that is not married
    /// filing jointly.
    fn test_config() -> QbiWorksheetConfig {
        QbiWorksheetConfig {
            deduction_rate: dec!(0.20),
            taxable_income_threshold: Some(dec!(197300)),
        }
    }

    fn test_worksheet() -> QbiWorksheet {
        QbiWorksheet::new(test_config())
    }

    // =========================================================================
    // QbiWorksheetConfig::validate tests
    // =========================================================================

    #[test]
    fn validate_accepts_valid_config() {
        assert_eq!(test_config().validate(), Ok(()));
    }

    #[test]
    fn validate_accepts_missing_threshold() {
        let config = QbiWorksheetConfig {
            taxable_income_threshold: None,
            ..test_config()
        };

        assert_eq!(config.validate(), Ok(()));
    }

    #[test]
    fn validate_rejects_negative_deduction_rate() {
        let config = QbiWorksheetConfig {
            deduction_rate: dec!(-0.1),
            ..test_config()
        };

        assert_eq!(
            config.validate(),
            Err(QbiWorksheetError::InvalidDeductionRate(dec!(-0.1)))
        );
    }

    #[test]
    fn validate_rejects_deduction_rate_greater_than_one() {
        let config = QbiWorksheetConfig {
            deduction_rate: dec!(1.5),
            ..test_config()
        };

        assert_eq!(
            config.validate(),
            Err(QbiWorksheetError::InvalidDeductionRate(dec!(1.5)))
        );
    }

    #[test]
    fn validate_rejects_negative_threshold() {
        let config = QbiWorksheetConfig {
            taxable_income_threshold: Some(dec!(-1)),
            ..test_config()
        };

        assert_eq!(
            config.validate(),
            Err(QbiWorksheetError::InvalidTaxableIncomeThreshold(dec!(-1)))
        );
    }

    // =========================================================================
    // total_qbi_or_loss tests (Line 2)
    // =========================================================================

    #[test]
    fn total_qbi_or_loss_of_no_entries_is_zero() {
        assert_eq!(test_worksheet().total_qbi_or_loss(&[]), Decimal::ZERO);
    }

    #[test]
    fn total_qbi_or_loss_is_reduced_by_negative_entries() {
        let result = test_worksheet().total_qbi_or_loss(&[dec!(250), dec!(-75)]);

        assert_eq!(result, dec!(175));
    }

    #[test]
    fn total_qbi_or_loss_is_summed_without_rounding() {
        let result = test_worksheet().total_qbi_or_loss(&[dec!(1.10), dec!(2.20), dec!(3.30)]);

        assert_eq!(result, dec!(6.60));
    }

    // =========================================================================
    // qbi_component tests (Line 5)
    // =========================================================================

    #[test]
    fn qbi_component_rounds_half_up() {
        // 0.625 × 0.20 = 0.125, rounds to 0.13
        assert_eq!(test_worksheet().qbi_component(dec!(0.625)), dec!(0.13));
    }

    // =========================================================================
    // exceeds_threshold tests
    // =========================================================================

    #[test]
    fn exceeds_threshold_only_above_the_limit() {
        let worksheet = test_worksheet();

        assert!(worksheet.exceeds_threshold(Some(dec!(197301))));
        assert!(!worksheet.exceeds_threshold(Some(dec!(197300))));
        assert!(!worksheet.exceeds_threshold(Some(dec!(197299))));
    }

    #[test]
    fn exceeds_threshold_is_false_without_both_values() {
        let no_threshold = QbiWorksheet::new(QbiWorksheetConfig {
            taxable_income_threshold: None,
            ..test_config()
        });

        assert!(!test_worksheet().exceeds_threshold(None));
        assert!(!no_threshold.exceeds_threshold(Some(dec!(500000))));
    }

    // =========================================================================
    // calculate (integration) tests
    // =========================================================================

    #[test]
    fn calculate_sets_every_line_when_all_inputs_are_entered() {
        let input = QbiWorksheetInput {
            trade_or_business_income: vec![dec!(70000.50)],
            taxable_income_before_qbi: Some(dec!(80000.25)),
            net_capital_gain: dec!(550.75),
            ..QbiWorksheetInput::default()
        };

        assert_eq!(
            test_worksheet().calculate(&input),
            Ok(QbiWorksheetResult {
                total_qbi_or_loss: dec!(70000.50),
                total_qbi: dec!(70000.50),
                qbi_component: dec!(14000.10),
                total_reit_ptp_income: Decimal::ZERO,
                reit_ptp_component: Decimal::ZERO,
                deduction_before_income_limitation: dec!(14000.10),
                taxable_income_before_qbi: Some(dec!(80000.25)),
                taxable_income_less_net_capital_gain: Some(dec!(79449.50)),
                income_limitation: Some(dec!(15889.90)),
                qbi_deduction: dec!(14000.10),
                total_qbi_loss_carryforward: Decimal::ZERO,
                total_reit_ptp_loss_carryforward: Decimal::ZERO,
                above_threshold: false,
            })
        );
    }

    #[test]
    fn calculate_adds_reit_and_ptp_amounts_to_the_deduction() {
        let input = QbiWorksheetInput {
            trade_or_business_income: vec![dec!(10000)],
            reit_ptp_income: dec!(5000),
            reit_ptp_loss_carryforward: dec!(-1000),
            taxable_income_before_qbi: Some(dec!(100000)),
            ..QbiWorksheetInput::default()
        };

        assert_eq!(
            test_worksheet().calculate(&input),
            Ok(QbiWorksheetResult {
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
                total_reit_ptp_loss_carryforward: Decimal::ZERO,
                above_threshold: false,
            })
        );
    }

    #[test]
    fn calculate_carries_losses_forward_and_zeroes_current_year_amounts() {
        let input = QbiWorksheetInput {
            trade_or_business_income: vec![dec!(1000)],
            qbi_loss_carryforward: dec!(-3000),
            reit_ptp_income: dec!(-400),
            taxable_income_before_qbi: Some(dec!(10000)),
            ..QbiWorksheetInput::default()
        };

        assert_eq!(
            test_worksheet().calculate(&input),
            Ok(QbiWorksheetResult {
                total_qbi_or_loss: dec!(1000),
                total_qbi: Decimal::ZERO,
                qbi_component: Decimal::ZERO,
                total_reit_ptp_income: Decimal::ZERO,
                reit_ptp_component: Decimal::ZERO,
                deduction_before_income_limitation: Decimal::ZERO,
                taxable_income_before_qbi: Some(dec!(10000)),
                taxable_income_less_net_capital_gain: Some(dec!(10000)),
                income_limitation: Some(dec!(2000)),
                qbi_deduction: Decimal::ZERO,
                total_qbi_loss_carryforward: dec!(-2000),
                total_reit_ptp_loss_carryforward: dec!(-400),
                above_threshold: false,
            })
        );
    }

    #[test]
    fn calculate_caps_the_deduction_at_the_income_limitation() {
        let input = QbiWorksheetInput {
            trade_or_business_income: vec![dec!(100000)],
            taxable_income_before_qbi: Some(dec!(10000)),
            ..QbiWorksheetInput::default()
        };

        assert_eq!(
            test_worksheet().calculate(&input),
            Ok(QbiWorksheetResult {
                total_qbi_or_loss: dec!(100000),
                total_qbi: dec!(100000),
                qbi_component: dec!(20000),
                total_reit_ptp_income: Decimal::ZERO,
                reit_ptp_component: Decimal::ZERO,
                deduction_before_income_limitation: dec!(20000),
                taxable_income_before_qbi: Some(dec!(10000)),
                taxable_income_less_net_capital_gain: Some(dec!(10000)),
                income_limitation: Some(dec!(2000)),
                qbi_deduction: dec!(2000),
                total_qbi_loss_carryforward: Decimal::ZERO,
                total_reit_ptp_loss_carryforward: Decimal::ZERO,
                above_threshold: false,
            })
        );
    }

    #[test]
    fn calculate_leaves_the_limitation_blank_without_taxable_income() {
        let input = QbiWorksheetInput {
            trade_or_business_income: vec![dec!(100000)],
            ..QbiWorksheetInput::default()
        };

        assert_eq!(
            test_worksheet().calculate(&input),
            Ok(QbiWorksheetResult {
                total_qbi_or_loss: dec!(100000),
                total_qbi: dec!(100000),
                qbi_component: dec!(20000),
                total_reit_ptp_income: Decimal::ZERO,
                reit_ptp_component: Decimal::ZERO,
                deduction_before_income_limitation: dec!(20000),
                taxable_income_before_qbi: None,
                taxable_income_less_net_capital_gain: None,
                income_limitation: None,
                qbi_deduction: Decimal::ZERO,
                total_qbi_loss_carryforward: Decimal::ZERO,
                total_reit_ptp_loss_carryforward: Decimal::ZERO,
                above_threshold: false,
            })
        );
    }

    #[test]
    fn calculate_reports_income_above_the_threshold() {
        let input = QbiWorksheetInput {
            trade_or_business_income: vec![dec!(50000)],
            taxable_income_before_qbi: Some(dec!(197300.01)),
            ..QbiWorksheetInput::default()
        };

        let result = test_worksheet().calculate(&input).unwrap();

        assert!(result.above_threshold);
        // The lines are still figured so the form can show them.
        assert_eq!(result.qbi_deduction, dec!(10000));
    }

    #[test]
    fn calculate_returns_error_for_invalid_config() {
        let worksheet = QbiWorksheet::new(QbiWorksheetConfig {
            deduction_rate: dec!(2),
            ..test_config()
        });

        assert_eq!(
            worksheet.calculate(&QbiWorksheetInput::default()),
            Err(QbiWorksheetError::InvalidDeductionRate(dec!(2)))
        );
    }
}
