use std::fmt::{self, Display, Formatter, Write};

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::calculations::{EstimatedTaxWorksheetContext, EstimatedTaxWorksheetInput};
use crate::db::TaxRecord;
use crate::models::FilingStatusCode;

/// Canonical user-entered estimate data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaxEstimateInput {
    pub tax_year: i32,
    pub filing_status: FilingStatusCode,

    pub se_income: Option<Decimal>,
    pub expected_crp_payments: Option<Decimal>,
    pub expected_wages: Option<Decimal>,

    pub expected_agi: Decimal,
    pub expected_deduction: Decimal,
    pub expected_qbi_deduction: Option<Decimal>,
    pub expected_amt: Option<Decimal>,
    pub expected_credits: Option<Decimal>,
    pub expected_other_taxes: Option<Decimal>,
    pub expected_withholding: Option<Decimal>,
    pub prior_year_tax: Option<Decimal>,
}

/// Stored calculated values for a persisted estimate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaxEstimateComputed {
    pub se_tax: Decimal,
    pub total_tax: Decimal,
    pub required_payment: Decimal,
}

/// Full persisted estimate record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaxEstimate {
    pub id: i64,
    pub input: TaxEstimateInput,
    pub computed: Option<TaxEstimateComputed>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Filter for listing estimates, optionally narrowed by year.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TaxEstimateFilter {
    pub tax_year: Option<i32>,
}

impl TaxRecord for TaxEstimate {
    type Key = i64;
    type Draft = TaxEstimateInput;
    type Filter = TaxEstimateFilter;
}

/// Tax year range accepted for estimates (inclusive).
const TAX_YEAR_MIN: i32 = 2000;
const TAX_YEAR_MAX: i32 = 2030;

impl TaxEstimateInput {
    /// Validates business rules before persistence or calculation.
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

    /// Resolves user-entered estimate data into worksheet-specific calculator input.
    pub fn to_estimated_tax_worksheet_input(
        &self,
        context: &EstimatedTaxWorksheetContext,
    ) -> EstimatedTaxWorksheetInput {
        EstimatedTaxWorksheetInput {
            adjusted_gross_income: self.expected_agi,
            deduction: self.expected_deduction,
            qbi_deduction: self.expected_qbi_deduction.unwrap_or_default(),
            alternative_minimum_tax: self.expected_amt.unwrap_or_default(),
            credits: self.expected_credits.unwrap_or_default(),
            self_employment_tax: context.self_employment_tax,
            other_taxes: self.expected_other_taxes.unwrap_or_default(),
            refundable_credits: context.refundable_credits,
            prior_year_tax: self.prior_year_tax.unwrap_or_default(),
            withholding: self.expected_withholding.unwrap_or_default(),
            is_farmer_or_fisher: context.is_farmer_or_fisher,
            required_payment_threshold: context.required_payment_threshold,
        }
    }
}

impl Display for TaxEstimateInput {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> fmt::Result {
        write!(
            f,
            "Tax estimate {}: filing status {}",
            self.tax_year,
            self.filing_status.as_str()
        )?;
        write!(f, ", se_income: ")?;
        fmt_opt_decimal(f, self.se_income.as_ref())?;
        write!(f, ", crp_payments: ")?;
        fmt_opt_decimal(f, self.expected_crp_payments.as_ref())?;
        write!(f, ", wages: ")?;
        fmt_opt_decimal(f, self.expected_wages.as_ref())?;
        write!(
            f,
            ", AGI {}, deduction {}",
            self.expected_agi, self.expected_deduction
        )?;
        write!(f, ", qbi_deduction: ")?;
        fmt_opt_decimal(f, self.expected_qbi_deduction.as_ref())?;
        write!(f, ", amt: ")?;
        fmt_opt_decimal(f, self.expected_amt.as_ref())?;
        write!(f, ", credits: ")?;
        fmt_opt_decimal(f, self.expected_credits.as_ref())?;
        write!(f, ", other_taxes: ")?;
        fmt_opt_decimal(f, self.expected_other_taxes.as_ref())?;
        write!(f, ", withholding: ")?;
        fmt_opt_decimal(f, self.expected_withholding.as_ref())?;
        write!(f, ", prior_year_tax: ")?;
        fmt_opt_decimal(f, self.prior_year_tax.as_ref())?;
        Ok(())
    }
}

fn fmt_opt_decimal(
    f: &mut impl Write,
    value: Option<&Decimal>,
) -> fmt::Result {
    match value {
        Some(d) => write!(f, "{d}"),
        None => write!(f, "—"),
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use rust_decimal_macros::dec;

    use super::*;

    fn valid_input() -> TaxEstimateInput {
        TaxEstimateInput {
            tax_year: 2025,
            filing_status: FilingStatusCode::Single,
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
    fn validate_for_submit_accepts_valid_input() {
        let input = valid_input();
        assert!(input.validate_for_submit().is_ok());
    }

    #[test]
    fn validate_for_submit_rejects_tax_year_below_min() {
        let mut input = valid_input();
        input.tax_year = TAX_YEAR_MIN - 1;
        let err = input
            .validate_for_submit()
            .expect_err("expected validation error");
        assert_eq!(err.len(), 1);
        assert!(err[0].contains("Tax year must be between"));
    }

    #[test]
    fn validate_for_submit_rejects_negative_expected_deduction() {
        let mut input = valid_input();
        input.expected_deduction = dec!(-1.00);
        let err = input
            .validate_for_submit()
            .expect_err("expected validation error");
        assert_eq!(err, vec!["Expected deduction cannot be negative"]);
    }

    #[test]
    fn worksheet_mapping_uses_expected_deduction_amount() {
        let mut input = valid_input();
        input.expected_deduction = dec!(15000.00);
        let context = EstimatedTaxWorksheetContext {
            self_employment_tax: dec!(1000.00),
            refundable_credits: dec!(250.00),
            is_farmer_or_fisher: false,
            required_payment_threshold: dec!(1000.00),
        };

        let worksheet_input = input.to_estimated_tax_worksheet_input(&context);

        assert_eq!(worksheet_input.deduction, dec!(15000.00));
        assert_eq!(worksheet_input.self_employment_tax, dec!(1000.00));
        assert_eq!(worksheet_input.refundable_credits, dec!(250.00));
        assert_eq!(worksheet_input.required_payment_threshold, dec!(1000.00));
    }

    #[test]
    fn fmt_opt_decimal_writes_value_when_some() {
        let d = Decimal::from(12345);
        let mut s = String::new();
        let result = fmt_opt_decimal(&mut s, Some(&d));
        assert!(result.is_ok());
        assert_eq!(s, "12345");
    }

    #[test]
    fn fmt_opt_decimal_writes_em_dash_when_none() {
        let mut s = String::new();
        let result = fmt_opt_decimal(&mut s, None);
        assert!(result.is_ok());
        assert_eq!(s, "—");
    }
}
