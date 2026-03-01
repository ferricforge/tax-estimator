use std::fmt::{self, Display, Formatter, Write};

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaxEstimate {
    pub id: i64,
    pub tax_year: i32,

    // User-provided values
    pub filing_status_id: i32,

    // User-provided values (SE Worksheet inputs)
    pub se_income: Option<Decimal>,
    pub expected_crp_payments: Option<Decimal>,
    pub expected_wages: Option<Decimal>,

    // User-provided values (1040-ES Worksheet inputs)
    pub expected_agi: Decimal,
    pub expected_deduction: Decimal,
    pub expected_qbi_deduction: Option<Decimal>,
    pub expected_amt: Option<Decimal>,
    pub expected_credits: Option<Decimal>,
    pub expected_other_taxes: Option<Decimal>,
    pub expected_withholding: Option<Decimal>,
    pub prior_year_tax: Option<Decimal>,

    // Calculated values
    pub calculated_se_tax: Option<Decimal>,
    pub calculated_total_tax: Option<Decimal>,
    pub calculated_required_payment: Option<Decimal>,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// For creating new estimates (no id or timestamps)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewTaxEstimate {
    pub tax_year: i32,
    pub filing_status_id: i32,

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

fn fmt_opt_decimal(
    f: &mut impl Write,
    value: Option<&Decimal>,
) -> fmt::Result {
    match value {
        Some(d) => write!(f, "{}", d),
        None => write!(f, "—"),
    }
}

impl Display for NewTaxEstimate {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> fmt::Result {
        write!(
            f,
            "Tax estimate {}: filing status {}",
            self.tax_year, self.filing_status_id
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

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn fmt_opt_decimal_writes_decimal_with_fractional_part() {
        let d = Decimal::try_from(1234.56).unwrap();
        let mut s = String::new();
        let result = fmt_opt_decimal(&mut s, Some(&d));
        assert!(result.is_ok());
        assert_eq!(s, "1234.56");
    }

    #[test]
    fn fmt_opt_decimal_writes_zero_when_some_zero() {
        let d = Decimal::ZERO;
        let mut s = String::new();
        let result = fmt_opt_decimal(&mut s, Some(&d));
        assert!(result.is_ok());
        assert_eq!(s, "0");
    }

    #[test]
    fn fmt_opt_decimal_writes_to_empty_string() {
        let d = Decimal::from(1);
        let mut s = String::new();
        assert!(s.is_empty());
        let result = fmt_opt_decimal(&mut s, Some(&d));
        assert!(result.is_ok());
        assert_eq!(s, "1");
    }

    struct FailingWriter;

    impl Write for FailingWriter {
        fn write_str(
            &mut self,
            _s: &str,
        ) -> fmt::Result {
            Err(fmt::Error)
        }
    }

    #[test]
    fn fmt_opt_decimal_propagates_write_error_on_some() {
        let d = Decimal::from(100);
        let mut w = FailingWriter;
        let result = fmt_opt_decimal(&mut w, Some(&d));
        assert!(result.is_err());
    }

    #[test]
    fn fmt_opt_decimal_propagates_write_error_on_none() {
        let mut w = FailingWriter;
        let result = fmt_opt_decimal(&mut w, None);
        assert!(result.is_err());
    }
}
