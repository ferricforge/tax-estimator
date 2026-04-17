use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::db::TaxRecord;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StandardDeduction {
    pub tax_year: i32,
    pub filing_status_id: i32,
    pub amount: Decimal,
}

impl TaxRecord for StandardDeduction {
    /// `(tax_year, filing_status_id)`
    type Key = (i32, i32);
    type Draft = StandardDeduction;
    /// Filter by `tax_year`.
    type Filter = i32;
}
