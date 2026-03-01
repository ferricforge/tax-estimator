use std::fmt;

use rust_decimal::Decimal;
use tax_core::{FilingStatusCode, NewTaxEstimate};

use crate::utils::opt_decimal_display;

/// Represents the collected values from the file selection form.
#[derive(Clone, Debug, Default)]
pub struct EstimatedIncomeModel {
    pub tax_year: i32,

    // User-provided values (1040-ES Worksheet inputs)
    pub filing_status_id: FilingStatusCode,
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

    pub fn to_new_tax_estimate(&self) -> NewTaxEstimate {
        NewTaxEstimate {
            tax_year: self.tax_year,
            filing_status_id: FilingStatusCode::filing_status_to_id(self.filing_status_id),
            expected_agi: self.expected_agi,
            expected_deduction: self.expected_deduction,
            expected_qbi_deduction: self.expected_qbi_deduction,
            expected_amt: self.expected_amt,
            expected_credits: self.expected_credits,
            expected_other_taxes: self.expected_other_taxes,
            expected_withholding: self.expected_withholding,
            prior_year_tax: self.prior_year_tax,
            se_income: self.se_income,
            expected_crp_payments: self.expected_crp_payments,
            expected_wages: self.expected_wages,
        }
    }
}

impl fmt::Display for EstimatedIncomeModel {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        writeln!(f, "Tax Year:           {}", self.tax_year)?;
        writeln!(
            f,
            "Filing status:     {}",
            self.filing_status_id.to_long_str()
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
