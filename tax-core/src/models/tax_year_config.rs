use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaxYearConfig {
    pub tax_year: i32,
    pub ss_wage_max: Decimal,
    pub ss_tax_rate: Decimal,
    pub medicare_tax_rate: Decimal,
    pub se_tax_deductible_percentage: Decimal,
    pub se_deduction_factor: Decimal,
    pub required_payment_threshold: Decimal,
    pub min_se_threshold: Decimal,
}
