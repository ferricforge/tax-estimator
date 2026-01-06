use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EstimatedTaxCalculation {
    pub id: i64,
    pub tax_year: i32,
    pub filing_status_id: i32,

    // 1040-ES Worksheet inputs
    pub expected_agi: Decimal,
    pub expected_deduction: Decimal,
    pub expected_qbi_deduction: Option<Decimal>,
    pub expected_amt: Option<Decimal>,
    pub expected_credits: Option<Decimal>,
    pub expected_other_taxes: Option<Decimal>,
    pub prior_year_tax: Option<Decimal>,
    pub expected_withholding: Option<Decimal>,

    // SE Worksheet inputs
    pub se_income: Option<Decimal>,
    pub expected_crp_payments: Option<Decimal>,
    pub expected_wages: Option<Decimal>,

    // Calculated values
    pub calculated_se_tax: Option<Decimal>,
    pub calculated_total_tax: Option<Decimal>,
    pub calculated_required_payment: Option<Decimal>,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// For creating new calculations (no id or timestamps)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewEstimatedTaxCalculation {
    pub tax_year: i32,
    pub filing_status_id: i32,
    pub expected_agi: Decimal,
    pub expected_deduction: Decimal,
    pub expected_qbi_deduction: Option<Decimal>,
    pub expected_amt: Option<Decimal>,
    pub expected_credits: Option<Decimal>,
    pub expected_other_taxes: Option<Decimal>,
    pub prior_year_tax: Option<Decimal>,
    pub expected_withholding: Option<Decimal>,
    pub se_income: Option<Decimal>,
    pub expected_crp_payments: Option<Decimal>,
    pub expected_wages: Option<Decimal>,
}
