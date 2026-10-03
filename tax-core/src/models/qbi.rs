//! Form 8995 data that is saved with a tax estimate.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::calculations::{QbiWorksheetInput, QbiWorksheetResult};

/// One trade, business, or aggregation from Form 8995, Line 1.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct QbiBusiness {
    /// Trade, business, or aggregation name (Line 1, column (a)).
    pub name: String,
    /// Taxpayer identification number (Line 1, column (b)).
    pub taxpayer_id: String,
    /// Qualified business income or (loss) (Line 1, column (c)).
    pub qualified_business_income: Decimal,
}

/// Canonical user-entered Form 8995 data.
///
/// Income is a positive number and a loss is a negative number, including the
/// loss carryforwards on Lines 3 and 7. This is the sign convention of
/// [`QbiWorksheetInput`], which this struct mirrors line for line.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct QbiInput {
    /// Line 1 rows, in the order they appear on the form.
    pub businesses: Vec<QbiBusiness>,
    /// Qualified business net (loss) carryforward from the prior year
    /// (Line 3).
    pub qbi_loss_carryforward: Decimal,
    /// Qualified REIT dividends and PTP income or (loss) (Line 6).
    pub reit_ptp_income: Decimal,
    /// Qualified REIT dividends and qualified PTP (loss) carryforward from
    /// the prior year (Line 7).
    pub reit_ptp_loss_carryforward: Decimal,
    /// Taxable income before the qualified business income deduction
    /// (Line 11). Figured from the estimate; `None` when not known yet.
    pub taxable_income_before_qbi: Option<Decimal>,
    /// Net capital gain, increased by any qualified dividends (Line 12).
    pub net_capital_gain: Decimal,
}

/// Stored calculated values for persisted Form 8995 data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QbiComputed {
    /// Qualified business income deduction (Line 15).
    pub qbi_deduction: Decimal,
    /// Total qualified business (loss) carryforward (Line 16).
    pub total_qbi_loss_carryforward: Decimal,
    /// Total qualified REIT dividends and PTP (loss) carryforward (Line 17).
    pub total_reit_ptp_loss_carryforward: Decimal,
}

/// Full persisted Form 8995 record. Belongs to one tax estimate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Qbi {
    pub tax_estimate_id: i64,
    pub input: QbiInput,
    pub computed: Option<QbiComputed>,
}

impl QbiInput {
    /// Validates business rules before persistence or calculation.
    pub fn validate_for_submit(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        for (index, business) in self.businesses.iter().enumerate() {
            let line = index + 1;
            if business.name.trim().is_empty() {
                errors.push(format!("Line 1 row {line}: business name is required"));
            }
            if business.taxpayer_id.trim().is_empty() {
                errors.push(format!(
                    "Line 1 row {line}: taxpayer identification number is required"
                ));
            }
        }
        if self.qbi_loss_carryforward > Decimal::ZERO {
            errors.push("QBI loss carryforward cannot be positive".to_string());
        }
        if self.reit_ptp_loss_carryforward > Decimal::ZERO {
            errors.push("REIT and PTP loss carryforward cannot be positive".to_string());
        }
        if self.net_capital_gain < Decimal::ZERO {
            errors.push("Net capital gain cannot be negative".to_string());
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

impl From<&QbiInput> for QbiWorksheetInput {
    fn from(input: &QbiInput) -> Self {
        Self {
            trade_or_business_income: input
                .businesses
                .iter()
                .map(|business| business.qualified_business_income)
                .collect(),
            qbi_loss_carryforward: input.qbi_loss_carryforward,
            reit_ptp_income: input.reit_ptp_income,
            reit_ptp_loss_carryforward: input.reit_ptp_loss_carryforward,
            taxable_income_before_qbi: input.taxable_income_before_qbi,
            net_capital_gain: input.net_capital_gain,
        }
    }
}

impl From<&QbiWorksheetResult> for QbiComputed {
    fn from(result: &QbiWorksheetResult) -> Self {
        Self {
            qbi_deduction: result.qbi_deduction,
            total_qbi_loss_carryforward: result.total_qbi_loss_carryforward,
            total_reit_ptp_loss_carryforward: result.total_reit_ptp_loss_carryforward,
        }
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use rust_decimal_macros::dec;

    use super::*;
    use crate::calculations::{QbiWorksheet, QbiWorksheetConfig};

    fn valid_input() -> QbiInput {
        QbiInput {
            businesses: vec![QbiBusiness {
                name: "Consulting".to_string(),
                taxpayer_id: "12-3456789".to_string(),
                qualified_business_income: dec!(70000.50),
            }],
            qbi_loss_carryforward: dec!(-3000.00),
            reit_ptp_income: dec!(500.00),
            reit_ptp_loss_carryforward: dec!(-100.00),
            taxable_income_before_qbi: Some(dec!(80000.25)),
            net_capital_gain: dec!(550.75),
        }
    }

    #[test]
    fn validate_for_submit_accepts_valid_input() {
        assert!(valid_input().validate_for_submit().is_ok());
    }

    #[test]
    fn validate_for_submit_requires_name_and_taxpayer_id() {
        let mut input = valid_input();
        input.businesses[0].name = " ".to_string();
        input.businesses[0].taxpayer_id = String::new();

        let err = input
            .validate_for_submit()
            .expect_err("expected validation error");

        assert_eq!(
            err,
            vec![
                "Line 1 row 1: business name is required",
                "Line 1 row 1: taxpayer identification number is required",
            ]
        );
    }

    #[test]
    fn validate_for_submit_rejects_positive_loss_carryforwards() {
        let mut input = valid_input();
        input.qbi_loss_carryforward = dec!(1.00);
        input.reit_ptp_loss_carryforward = dec!(1.00);

        let err = input
            .validate_for_submit()
            .expect_err("expected validation error");

        assert_eq!(
            err,
            vec![
                "QBI loss carryforward cannot be positive",
                "REIT and PTP loss carryforward cannot be positive",
            ]
        );
    }

    #[test]
    fn worksheet_input_copies_every_line() {
        assert_eq!(
            QbiWorksheetInput::from(&valid_input()),
            QbiWorksheetInput {
                trade_or_business_income: vec![dec!(70000.50)],
                qbi_loss_carryforward: dec!(-3000.00),
                reit_ptp_income: dec!(500.00),
                reit_ptp_loss_carryforward: dec!(-100.00),
                taxable_income_before_qbi: Some(dec!(80000.25)),
                net_capital_gain: dec!(550.75),
            }
        );
    }

    #[test]
    fn computed_takes_lines_15_16_and_17_from_the_result() {
        let worksheet = QbiWorksheet::new(QbiWorksheetConfig::for_tax_year(2025, false));
        let result = worksheet
            .calculate(&QbiWorksheetInput::from(&valid_input()))
            .unwrap();

        // Line 5: (70,000.50 - 3,000) × 0.20 = 13,400.10
        // Line 9: (500 - 100) × 0.20 = 80
        // Line 14: (80,000.25 - 550.75) × 0.20 = 15,889.90, above Line 10
        assert_eq!(
            QbiComputed::from(&result),
            QbiComputed {
                qbi_deduction: dec!(13480.10),
                total_qbi_loss_carryforward: Decimal::ZERO,
                total_reit_ptp_loss_carryforward: Decimal::ZERO,
            }
        );
    }
}
