use std::fmt;

use rust_decimal::Decimal;
use tax_core::{FilingStatusCode, NewTaxEstimate};

use crate::utils::opt_decimal_display;

/// Represents the collected values from the file selection form.
#[derive(Clone, Debug, Default)]
pub struct EstimatedIncomeModel {
    pub tax_year: i32,

    // User-provided values
    pub filing_status_id: FilingStatusCode,

    // User-provided values (SE Worksheet inputs)
    pub se_income: Option<Decimal>,
    pub expected_crp_payments: Option<Decimal>,
    pub expected_wages: Option<Decimal>,

    // User-provided values (1040-ES Worksheet inputs)
    /// 2025 Estimated Tax Worksheet, line 1: adjusted gross income
    /// you expect for the year (see form instructions).
    pub expected_agi: Decimal,
    /// 2025 Estimated Tax Worksheet, line 2a: deductions
    /// (estimated itemized deductions or standard deduction).
    pub expected_deduction: Decimal,
    /// 2025 Estimated Tax Worksheet, line 2b: qualified
    /// business income deduction, if applicable.
    pub expected_qbi_deduction: Option<Decimal>,
    /// 2025 Estimated Tax Worksheet, line 5: alternative minimum tax
    /// from Form 6251.
    pub expected_amt: Option<Decimal>,
    /// 2025 Estimated Tax Worksheet, line 7: credits
    /// (do not include income tax withholding on this line).
    pub expected_credits: Option<Decimal>,
    /// 2025 Estimated Tax Worksheet, line 10: other taxes
    /// (see worksheet instructions).
    pub expected_other_taxes: Option<Decimal>,
    /// 2025 Estimated Tax Worksheet, line 13: income tax withheld and estimated
    /// to be withheld (including withholding on pensions, annuities, certain
    /// deferred income, and Additional Medicare Tax withholding).
    pub expected_withholding: Option<Decimal>,
    /// 2025 Estimated Tax Worksheet, line 12b: required annual payment based
    /// on prior year's tax (as figured per worksheet instructions).
    pub prior_year_tax: Option<Decimal>,
}

/// Tax year range accepted for estimates (inclusive).
const TAX_YEAR_MIN: i32 = 2000;
const TAX_YEAR_MAX: i32 = 2030;

impl EstimatedIncomeModel {
    /// Validates business rules before submission.
    ///
    /// Rules:
    /// - Tax year must be in the supported range (e.g. 2000..=2030).
    /// - Expected AGI and expected deduction must be non-negative.
    /// - Any optional decimal field that is present must be non-negative.
    pub fn validate_for_submit(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        if self.tax_year < TAX_YEAR_MIN || self.tax_year > TAX_YEAR_MAX {
            errors.push(format!(
                "Tax year must be between {} and {}",
                TAX_YEAR_MIN, TAX_YEAR_MAX
            ));
        }

        if self.expected_agi < Decimal::ZERO {
            errors.push("Expected AGI cannot be negative".to_string());
        }
        if self.expected_deduction < Decimal::ZERO {
            errors.push("Expected deduction cannot be negative".to_string());
        }

        for (label, opt) in [
            ("SE income", &self.se_income),
            ("CRP payments", &self.expected_crp_payments),
            ("Wages", &self.expected_wages),
            ("QBI deduction", &self.expected_qbi_deduction),
            ("AMT", &self.expected_amt),
            ("Credits", &self.expected_credits),
            ("Other taxes", &self.expected_other_taxes),
            ("Withholding", &self.expected_withholding),
            ("Prior year tax", &self.prior_year_tax),
        ] {
            if let Some(d) = opt
                && *d < Decimal::ZERO
            {
                errors.push(format!("{label} cannot be negative"));
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    pub fn to_new_tax_estimate(&self) -> NewTaxEstimate {
        NewTaxEstimate {
            tax_year: self.tax_year,
            filing_status_id: self.filing_status_id.filing_status_to_id(),
            se_income: self.se_income,
            expected_crp_payments: self.expected_crp_payments,
            expected_wages: self.expected_wages,
            expected_agi: self.expected_agi,
            expected_deduction: self.expected_deduction,
            expected_qbi_deduction: self.expected_qbi_deduction,
            expected_amt: self.expected_amt,
            expected_credits: self.expected_credits,
            expected_other_taxes: self.expected_other_taxes,
            expected_withholding: self.expected_withholding,
            prior_year_tax: self.prior_year_tax,
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
        writeln!(
            f,
            "Wages:              {}",
            opt_decimal_display(&self.expected_wages)
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
        write!(
            f,
            "Prior year tax:     {}",
            opt_decimal_display(&self.prior_year_tax)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_test_model() -> EstimatedIncomeModel {
        EstimatedIncomeModel {
            tax_year: 2025,
            filing_status_id: FilingStatusCode::Single,
            se_income: None,
            expected_crp_payments: None,
            expected_wages: None,
            expected_agi: Decimal::ZERO,
            expected_deduction: Decimal::ZERO,
            expected_qbi_deduction: None,
            expected_amt: None,
            expected_credits: None,
            expected_other_taxes: None,
            expected_withholding: None,
            prior_year_tax: None,
        }
    }

    #[test]
    fn validate_for_submit_accepts_valid_model() {
        let m = valid_test_model();
        assert!(m.validate_for_submit().is_ok());
    }

    #[test]
    fn validate_for_submit_rejects_tax_year_below_min() {
        let mut m = valid_test_model();
        m.tax_year = TAX_YEAR_MIN - 1;
        let err = m.validate_for_submit().unwrap_err();
        assert_eq!(err.len(), 1);
        assert!(err[0].contains("Tax year must be between"));
    }

    #[test]
    fn validate_for_submit_rejects_tax_year_above_max() {
        let mut m = valid_test_model();
        m.tax_year = TAX_YEAR_MAX + 1;
        let err = m.validate_for_submit().unwrap_err();
        assert_eq!(err.len(), 1);
        assert!(err[0].contains("Tax year must be between"));
    }

    #[test]
    fn validate_for_submit_rejects_negative_agi() {
        let mut m = valid_test_model();
        m.expected_agi = Decimal::from(-1);
        let err = m.validate_for_submit().unwrap_err();
        assert_eq!(err.len(), 1);
        assert_eq!(err[0], "Expected AGI cannot be negative");
    }

    #[test]
    fn validate_for_submit_rejects_negative_optional_decimal() {
        let mut m = valid_test_model();
        m.se_income = Some(Decimal::from(-100));
        let err = m.validate_for_submit().unwrap_err();
        assert_eq!(err.len(), 1);
        assert_eq!(err[0], "SE income cannot be negative");
    }
}
