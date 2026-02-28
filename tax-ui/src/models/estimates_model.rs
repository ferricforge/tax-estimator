use std::fmt;

use rust_decimal::Decimal;

use crate::utils::opt_decimal_display;

/// Represents the collected values from the file selection form.
#[derive(Clone, Debug, Default)]
pub struct EstimatedIncomeModel {
    // User-provided values (1040-ES Worksheet inputs)
    pub filing_status_id: Option<String>,
    pub expected_agi: Decimal,
    pub expected_deduction: Decimal,
    pub expected_qbi_deduction: Option<Decimal>,
    pub expected_amt: Option<Decimal>,
    pub expected_credits: Option<Decimal>,
    pub expected_other_taxes: Option<Decimal>,
    pub expected_withholding: Option<Decimal>,
    pub prior_year_tax: Option<Decimal>,

    // User-provided values (SE Worksheet inputs)
    pub se_income: Option<Decimal>,
    pub expected_crp_payments: Option<Decimal>,
    pub expected_wages: Option<Decimal>,
}

impl EstimatedIncomeModel {
    /// Validates that the model has all required values for submission.
    ///
    /// Rules:
    /// - source file is required
    /// - database file is required
    /// - selected sheet is required only for Excel source files
    pub fn validate_for_submit(&self) -> Result<(), Vec<String>> {
        let mut _errors = Vec::new();

        if _errors.is_empty() {
            Ok(())
        } else {
            Err(_errors)
        }
    }
}

impl fmt::Display for EstimatedIncomeModel {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        writeln!(
            f,
            "Filing status:     {}",
            self.filing_status_id.as_deref().unwrap_or("—")
        )?;
        writeln!(f, "Expected AGI:       {}", self.expected_agi)?;
        writeln!(f, "Expected deduction: {}", self.expected_deduction)?;
        writeln!(
            f,
            "QBI deduction:      {}",
            opt_decimal_display(&self.expected_qbi_deduction)
        )?;
        writeln!(
            f,
            "AMT:                {}",
            opt_decimal_display(&self.expected_amt)
        )?;
        writeln!(
            f,
            "Credits:            {}",
            opt_decimal_display(&self.expected_credits)
        )?;
        writeln!(
            f,
            "Other taxes:        {}",
            opt_decimal_display(&self.expected_other_taxes)
        )?;
        writeln!(
            f,
            "Withholding:        {}",
            opt_decimal_display(&self.expected_withholding)
        )?;
        writeln!(
            f,
            "Prior year tax:     {}",
            opt_decimal_display(&self.prior_year_tax)
        )?;
        writeln!(
            f,
            "SE income:          {}",
            opt_decimal_display(&self.se_income)
        )?;
        writeln!(
            f,
            "CRP payments:       {}",
            opt_decimal_display(&self.expected_crp_payments)
        )?;
        write!(
            f,
            "Wages:              {}",
            opt_decimal_display(&self.expected_wages)
        )
    }
}
