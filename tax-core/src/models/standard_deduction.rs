use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StandardDeduction {
    pub tax_year: i32,
    pub filing_status_id: i32,
    pub amount: Decimal,
}
