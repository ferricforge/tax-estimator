use std::fmt::Display;

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaxYearConfig {
    pub tax_year: i32,
    pub ss_wage_max: Decimal,
    pub ss_tax_rate: Decimal,
    pub medicare_tax_rate: Decimal,
    pub se_tax_deduct_pcnt: Decimal,
    pub se_deduction_factor: Decimal,
    pub req_pmnt_threshold: Decimal,
    pub min_se_threshold: Decimal,
}


impl Display for TaxYearConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "TaxYearConfig {{")?;
        writeln!(f, "    tax_year            : {}", self.tax_year)?;
        writeln!(f, "    ss_wage_max         : {}", self.ss_wage_max)?;
        writeln!(f, "    ss_tax_rate         : {}", self.ss_tax_rate)?;
        writeln!(f, "    medicare_tax_rate   : {}", self.medicare_tax_rate)?;
        writeln!(f, "    se_tax_deduct_pcnt  : {}", self.se_tax_deduct_pcnt)?;
        writeln!(f, "    se_deduction_factor : {}", self.se_deduction_factor)?;
        writeln!(f, "    req_pmnt_threshold  : {}", self.req_pmnt_threshold)?;
        writeln!(f, "    min_se_threshold    : {}", self.min_se_threshold)?;
        write!(f, "}}")?;

        Ok(())
    }
}