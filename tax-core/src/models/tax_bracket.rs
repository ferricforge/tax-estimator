use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::db::TaxRecord;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaxBracket {
    pub tax_year: i32,
    pub filing_status_id: i32,
    pub min_income: Decimal,
    pub max_income: Option<Decimal>,
    pub tax_rate: Decimal,
    pub base_tax: Decimal,
}

/// Filter for listing or deleting brackets by year and filing status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaxBracketFilter {
    pub tax_year: i32,
    pub filing_status_id: i32,
}

impl TaxRecord for TaxBracket {
    /// `(tax_year, filing_status_id, min_income)`
    type Key = (i32, i32, Decimal);
    type Draft = TaxBracket;
    type Filter = TaxBracketFilter;
}
