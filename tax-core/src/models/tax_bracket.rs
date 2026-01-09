use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaxBracket {
    pub tax_year: i32,
    pub filing_status_id: i32,
    pub min_income: Decimal,
    pub max_income: Option<Decimal>,
    pub tax_rate: Decimal,
    pub base_tax: Decimal,
}
