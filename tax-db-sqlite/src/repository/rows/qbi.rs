use rust_decimal::Decimal;
use sqlx::{FromRow, sqlite::SqliteRow};
use tax_core::{Qbi, QbiBusiness, QbiComputed, QbiInput, RepositoryError};

use super::{decode_error, field};
use crate::decimal::{get_decimal, get_optional_decimal};

/// One row of `qbi` LEFT JOINed to `qbi_business`.
///
/// The `qbi` columns repeat on every joined row. `business` holds the Line 1
/// row carried by this joined row, or `None` when the estimate has no Line 1
/// rows (the join then yields a single row with NULL business columns).
pub(crate) struct QbiRow {
    tax_estimate_id: i64,
    qbi_loss_carryforward: Decimal,
    reit_ptp_income: Decimal,
    reit_ptp_loss_carryforward: Decimal,
    taxable_income_before_qbi: Option<Decimal>,
    net_capital_gain: Decimal,
    calculated_qbi_deduction: Option<Decimal>,
    calculated_qbi_loss_carryforward: Option<Decimal>,
    calculated_reit_ptp_loss_carryforward: Option<Decimal>,
    business: Option<QbiBusiness>,
}

impl QbiRow {
    fn decode(row: &SqliteRow) -> Result<Self, RepositoryError> {
        // `line_number` is part of the primary key, so it is NULL only when
        // the LEFT JOIN found no `qbi_business` row.
        let line_number: Option<i64> = field(row, "line_number")?;
        let business = match line_number {
            Some(_) => Some(QbiBusiness {
                name: field(row, "business_name")?,
                taxpayer_id: field(row, "taxpayer_id")?,
                qualified_business_income: get_decimal(row, "qualified_business_income")?,
            }),
            None => None,
        };

        Ok(Self {
            tax_estimate_id: field(row, "tax_estimate_id")?,
            qbi_loss_carryforward: get_decimal(row, "qbi_loss_carryforward")?,
            reit_ptp_income: get_decimal(row, "reit_ptp_income")?,
            reit_ptp_loss_carryforward: get_decimal(row, "reit_ptp_loss_carryforward")?,
            taxable_income_before_qbi: get_optional_decimal(row, "taxable_income_before_qbi")?,
            net_capital_gain: get_decimal(row, "net_capital_gain")?,
            calculated_qbi_deduction: get_optional_decimal(row, "calculated_qbi_deduction")?,
            calculated_qbi_loss_carryforward: get_optional_decimal(
                row,
                "calculated_qbi_loss_carryforward",
            )?,
            calculated_reit_ptp_loss_carryforward: get_optional_decimal(
                row,
                "calculated_reit_ptp_loss_carryforward",
            )?,
            business,
        })
    }

    /// Combine the joined rows for one estimate into the domain model.
    ///
    /// Returns `Ok(None)` when `rows` is empty, which means no Form 8995 data
    /// is stored for the estimate. The rows must already be ordered by
    /// `line_number`.
    pub(crate) fn into_qbi(rows: Vec<Self>) -> Result<Option<Qbi>, RepositoryError> {
        let Some(first) = rows.first() else {
            return Ok(None);
        };

        let computed = match (
            first.calculated_qbi_deduction,
            first.calculated_qbi_loss_carryforward,
            first.calculated_reit_ptp_loss_carryforward,
        ) {
            (None, None, None) => None,
            (
                Some(qbi_deduction),
                Some(total_qbi_loss_carryforward),
                Some(total_reit_ptp_loss_carryforward),
            ) => Some(QbiComputed {
                qbi_deduction,
                total_qbi_loss_carryforward,
                total_reit_ptp_loss_carryforward,
            }),
            _ => {
                return Err(RepositoryError::InvalidData(
                    "qbi row has partially populated calculated fields".to_string(),
                ));
            }
        };

        let tax_estimate_id = first.tax_estimate_id;
        let mut input = QbiInput {
            businesses: Vec::with_capacity(rows.len()),
            qbi_loss_carryforward: first.qbi_loss_carryforward,
            reit_ptp_income: first.reit_ptp_income,
            reit_ptp_loss_carryforward: first.reit_ptp_loss_carryforward,
            taxable_income_before_qbi: first.taxable_income_before_qbi,
            net_capital_gain: first.net_capital_gain,
        };
        input
            .businesses
            .extend(rows.into_iter().filter_map(|row| row.business));

        Ok(Some(Qbi {
            tax_estimate_id,
            input,
            computed,
        }))
    }
}

impl<'r> FromRow<'r, SqliteRow> for QbiRow {
    fn from_row(row: &'r SqliteRow) -> Result<Self, sqlx::Error> {
        Self::decode(row).map_err(decode_error)
    }
}
