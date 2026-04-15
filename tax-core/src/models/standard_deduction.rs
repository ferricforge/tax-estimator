use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use tax_db_macros::Entity;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Entity)]
#[entity(table = "standard_deductions")]
pub struct StandardDeduction {
    #[entity(pk)]
    pub tax_year: i32,
    #[entity(pk)]
    pub filing_status_id: i32,
    #[entity(
        encode_with = "crate::encode::decimal_as_f64",
        decode_with = "crate::encode::decimal_from_sql"
    )]
    pub amount: Decimal,
}
