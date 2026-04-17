use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{Row, sqlite::SqliteRow};
use tax_core::{
    FilingStatusCode, Persist, RepositoryError, TaxEstimate, TaxEstimateComputed,
    TaxEstimateFilter, TaxEstimateInput,
};

use crate::SqliteRepository;
use crate::decimal::{decimal_to_f64, get_decimal, get_optional_decimal};
use crate::repository::db_err;

const SELECT: &str = "SELECT te.id, te.tax_year, te.expected_agi, te.expected_deduction,
        te.expected_qbi_deduction, te.expected_amt, te.expected_credits,
        te.expected_other_taxes, te.expected_withholding, te.prior_year_tax,
        te.se_income, te.expected_crp_payments, te.expected_wages,
        te.calculated_se_tax, te.calculated_total_tax, te.calculated_required_payment,
        te.created_at, te.updated_at, fs.status_code AS filing_status_code
     FROM tax_estimate te
     JOIN filing_status fs ON fs.id = te.filing_status_id";

fn from_row(row: &SqliteRow) -> Result<TaxEstimate, RepositoryError> {
    let code_str: String = row.try_get("filing_status_code").map_err(db_err)?;
    let filing_status = FilingStatusCode::parse(&code_str).ok_or_else(|| {
        RepositoryError::InvalidData(format!(
            "Invalid filing status code on tax_estimate row: {code_str}"
        ))
    })?;

    let computed = match (
        get_optional_decimal(row, "calculated_se_tax")?,
        get_optional_decimal(row, "calculated_total_tax")?,
        get_optional_decimal(row, "calculated_required_payment")?,
    ) {
        (None, None, None) => None,
        (Some(se_tax), Some(total_tax), Some(required_payment)) => Some(TaxEstimateComputed {
            se_tax,
            total_tax,
            required_payment,
        }),
        _ => {
            return Err(RepositoryError::InvalidData(
                "tax_estimate row has partially populated calculated fields".to_string(),
            ));
        }
    };

    Ok(TaxEstimate {
        id: row.try_get("id").map_err(db_err)?,
        input: TaxEstimateInput {
            tax_year: row.try_get("tax_year").map_err(db_err)?,
            filing_status,
            se_income: get_optional_decimal(row, "se_income")?,
            expected_crp_payments: get_optional_decimal(row, "expected_crp_payments")?,
            expected_wages: get_optional_decimal(row, "expected_wages")?,
            expected_agi: get_decimal(row, "expected_agi")?,
            expected_deduction: get_decimal(row, "expected_deduction")?,
            expected_qbi_deduction: get_optional_decimal(row, "expected_qbi_deduction")?,
            expected_amt: get_optional_decimal(row, "expected_amt")?,
            expected_credits: get_optional_decimal(row, "expected_credits")?,
            expected_other_taxes: get_optional_decimal(row, "expected_other_taxes")?,
            expected_withholding: get_optional_decimal(row, "expected_withholding")?,
            prior_year_tax: get_optional_decimal(row, "prior_year_tax")?,
        },
        computed,
        created_at: row.try_get::<DateTime<Utc>, _>("created_at").map_err(|e| {
            RepositoryError::Database(anyhow::anyhow!("Failed to get created_at: {e}"))
        })?,
        updated_at: row.try_get::<DateTime<Utc>, _>("updated_at").map_err(|e| {
            RepositoryError::Database(anyhow::anyhow!("Failed to get updated_at: {e}"))
        })?,
    })
}

#[async_trait]
impl Persist<TaxEstimate> for SqliteRepository {
    async fn fetch(
        &self,
        id: &i64,
    ) -> Result<TaxEstimate, RepositoryError> {
        let row = sqlx::query(&format!("{SELECT} WHERE te.id = ?"))
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(db_err)?
            .ok_or(RepositoryError::NotFound)?;
        from_row(&row)
    }

    async fn fetch_all(
        &self,
        filter: &TaxEstimateFilter,
    ) -> Result<Vec<TaxEstimate>, RepositoryError> {
        let rows = match filter.tax_year {
            Some(year) => {
                sqlx::query(&format!(
                    "{SELECT} WHERE te.tax_year = ? ORDER BY te.updated_at DESC"
                ))
                .bind(year)
                .fetch_all(&self.pool)
                .await
            }
            None => {
                sqlx::query(&format!("{SELECT} ORDER BY te.updated_at DESC"))
                    .fetch_all(&self.pool)
                    .await
            }
        }
        .map_err(db_err)?;
        rows.iter().map(from_row).collect()
    }

    async fn create(
        &self,
        draft: TaxEstimateInput,
    ) -> Result<TaxEstimate, RepositoryError> {
        let now: DateTime<Utc> = Utc::now();
        // Uses the static seed-id mapping rather than a round-trip lookup.
        let filing_status_id = draft.filing_status.filing_status_to_id();

        let id: i64 = sqlx::query_scalar(
            "INSERT INTO tax_estimate (
                tax_year, filing_status_id, expected_agi, expected_deduction,
                expected_qbi_deduction, expected_amt, expected_credits,
                expected_other_taxes, expected_withholding, prior_year_tax,
                se_income, expected_crp_payments, expected_wages,
                created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT (tax_year, filing_status_id) DO UPDATE SET
                expected_agi = excluded.expected_agi,
                expected_deduction = excluded.expected_deduction,
                expected_qbi_deduction = excluded.expected_qbi_deduction,
                expected_amt = excluded.expected_amt,
                expected_credits = excluded.expected_credits,
                expected_other_taxes = excluded.expected_other_taxes,
                expected_withholding = excluded.expected_withholding,
                prior_year_tax = excluded.prior_year_tax,
                se_income = excluded.se_income,
                expected_crp_payments = excluded.expected_crp_payments,
                expected_wages = excluded.expected_wages,
                calculated_se_tax = NULL,
                calculated_total_tax = NULL,
                calculated_required_payment = NULL,
                updated_at = excluded.updated_at
            RETURNING id",
        )
        .bind(draft.tax_year)
        .bind(filing_status_id)
        .bind(decimal_to_f64(draft.expected_agi))
        .bind(decimal_to_f64(draft.expected_deduction))
        .bind(draft.expected_qbi_deduction.map(decimal_to_f64))
        .bind(draft.expected_amt.map(decimal_to_f64))
        .bind(draft.expected_credits.map(decimal_to_f64))
        .bind(draft.expected_other_taxes.map(decimal_to_f64))
        .bind(draft.expected_withholding.map(decimal_to_f64))
        .bind(draft.prior_year_tax.map(decimal_to_f64))
        .bind(draft.se_income.map(decimal_to_f64))
        .bind(draft.expected_crp_payments.map(decimal_to_f64))
        .bind(draft.expected_wages.map(decimal_to_f64))
        .bind(now)
        .bind(now)
        .fetch_one(&self.pool)
        .await
        .map_err(db_err)?;

        Persist::<TaxEstimate>::fetch(self, &id).await
    }

    async fn update(
        &self,
        record: &TaxEstimate,
    ) -> Result<(), RepositoryError> {
        let now = Utc::now();
        let filing_status_id = record.input.filing_status.filing_status_to_id();
        let (se_tax, total_tax, required_payment) = match &record.computed {
            Some(c) => (
                Some(decimal_to_f64(c.se_tax)),
                Some(decimal_to_f64(c.total_tax)),
                Some(decimal_to_f64(c.required_payment)),
            ),
            None => (None, None, None),
        };

        let result = sqlx::query(
            "UPDATE tax_estimate SET
                tax_year = ?, filing_status_id = ?, expected_agi = ?, expected_deduction = ?,
                expected_qbi_deduction = ?, expected_amt = ?, expected_credits = ?,
                expected_other_taxes = ?, expected_withholding = ?, prior_year_tax = ?,
                se_income = ?, expected_crp_payments = ?, expected_wages = ?,
                calculated_se_tax = ?, calculated_total_tax = ?, calculated_required_payment = ?,
                updated_at = ?
             WHERE id = ?",
        )
        .bind(record.input.tax_year)
        .bind(filing_status_id)
        .bind(decimal_to_f64(record.input.expected_agi))
        .bind(decimal_to_f64(record.input.expected_deduction))
        .bind(record.input.expected_qbi_deduction.map(decimal_to_f64))
        .bind(record.input.expected_amt.map(decimal_to_f64))
        .bind(record.input.expected_credits.map(decimal_to_f64))
        .bind(record.input.expected_other_taxes.map(decimal_to_f64))
        .bind(record.input.expected_withholding.map(decimal_to_f64))
        .bind(record.input.prior_year_tax.map(decimal_to_f64))
        .bind(record.input.se_income.map(decimal_to_f64))
        .bind(record.input.expected_crp_payments.map(decimal_to_f64))
        .bind(record.input.expected_wages.map(decimal_to_f64))
        .bind(se_tax)
        .bind(total_tax)
        .bind(required_payment)
        .bind(now)
        .bind(record.id)
        .execute(&self.pool)
        .await
        .map_err(db_err)?;

        if result.rows_affected() == 0 {
            return Err(RepositoryError::NotFound);
        }
        Ok(())
    }

    async fn delete(
        &self,
        id: &i64,
    ) -> Result<(), RepositoryError> {
        let result = sqlx::query("DELETE FROM tax_estimate WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        if result.rows_affected() == 0 {
            return Err(RepositoryError::NotFound);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use rust_decimal_macros::dec;
    use tax_core::TaxRepository;

    use crate::repository::test_support::{clear_all_data, setup_test_db};

    use super::*;

    async fn seed(repo: &SqliteRepository) {
        clear_all_data(repo).await;
        sqlx::query(
            "INSERT INTO tax_year_config (
                tax_year, ss_wage_max, ss_tax_rate, medicare_tax_rate,
                se_tax_deductible_percentage, se_deduction_factor,
                required_payment_threshold, min_se_threshold
            ) VALUES
            (8888, 180000.00, 0.124, 0.029, 0.9235, 0.50, 1000.00, 400.00),
            (8887, 175000.00, 0.124, 0.029, 0.9235, 0.50, 1000.00, 400.00)",
        )
        .execute(repo.pool())
        .await
        .expect("tax_year_config");
        // create() now resolves filing_status_id via FilingStatusCode (Single → 1)
        // rather than a DB lookup, so the seeded id must match the canonical mapping.
        sqlx::query(
            "INSERT INTO filing_status (id, status_code, status_name)
             VALUES (1, 'S', 'Test Single')",
        )
        .execute(repo.pool())
        .await
        .expect("filing_status");
    }

    fn input() -> TaxEstimateInput {
        TaxEstimateInput {
            tax_year: 8888,
            filing_status: FilingStatusCode::Single,
            se_income: Some(dec!(50000.00)),
            expected_crp_payments: None,
            expected_wages: Some(dec!(50000.00)),
            expected_agi: dec!(100000.00),
            expected_deduction: dec!(15000.00),
            expected_qbi_deduction: Some(dec!(5000.00)),
            expected_amt: None,
            expected_credits: Some(dec!(2000.00)),
            expected_other_taxes: None,
            expected_withholding: Some(dec!(8000.00)),
            prior_year_tax: Some(dec!(12000.00)),
        }
    }

    fn minimal_input() -> TaxEstimateInput {
        TaxEstimateInput {
            tax_year: 8888,
            filing_status: FilingStatusCode::Single,
            se_income: None,
            expected_crp_payments: None,
            expected_wages: None,
            expected_agi: dec!(75000.00),
            expected_deduction: dec!(15000.00),
            expected_qbi_deduction: None,
            expected_amt: None,
            expected_credits: None,
            expected_other_taxes: None,
            expected_withholding: None,
            prior_year_tax: None,
        }
    }

    #[tokio::test]
    async fn create_and_get() {
        let repo = setup_test_db().await;
        seed(&repo).await;

        let created = TaxRepository::create::<TaxEstimate>(&repo, input())
            .await
            .expect("create");

        assert!(created.id > 0);
        assert_eq!(created.input.tax_year, 8888);
        assert_eq!(created.input.filing_status, FilingStatusCode::Single);
        assert_eq!(created.input.expected_agi, dec!(100000.00));
        assert_eq!(created.input.expected_deduction, dec!(15000.00));
        assert_eq!(created.input.expected_qbi_deduction, Some(dec!(5000.00)));
        assert_eq!(created.input.expected_amt, None);
        assert_eq!(created.input.expected_credits, Some(dec!(2000.00)));
        assert_eq!(created.input.expected_other_taxes, None);
        assert_eq!(created.input.expected_withholding, Some(dec!(8000.00)));
        assert_eq!(created.input.prior_year_tax, Some(dec!(12000.00)));
        assert_eq!(created.input.se_income, Some(dec!(50000.00)));
        assert_eq!(created.input.expected_crp_payments, None);
        assert_eq!(created.input.expected_wages, Some(dec!(50000.00)));
        assert_eq!(created.computed, None);

        let fetched = repo.get::<TaxEstimate>(&created.id).await.expect("get");
        assert_eq!(fetched.id, created.id);
        assert_eq!(fetched.input.expected_agi, dec!(100000.00));
    }

    #[tokio::test]
    async fn get_not_found() {
        let repo = setup_test_db().await;
        let result = repo.get::<TaxEstimate>(&99999).await;
        assert!(matches!(result, Err(RepositoryError::NotFound)));
    }

    #[tokio::test]
    async fn update_estimate() {
        let repo = setup_test_db().await;
        seed(&repo).await;

        let mut created = TaxRepository::create::<TaxEstimate>(&repo, minimal_input())
            .await
            .expect("create");

        created.input.expected_agi = dec!(150000.00);
        created.input.expected_deduction = dec!(18000.00);
        created.computed = Some(TaxEstimateComputed {
            se_tax: dec!(7500.00),
            total_tax: dec!(25000.00),
            required_payment: dec!(4000.00),
        });

        TaxRepository::update(&repo, &created)
            .await
            .expect("update");

        let fetched = repo.get::<TaxEstimate>(&created.id).await.expect("get");
        assert_eq!(fetched.input.expected_agi, dec!(150000.00));
        assert_eq!(fetched.input.expected_deduction, dec!(18000.00));
        assert_eq!(
            fetched.computed,
            Some(TaxEstimateComputed {
                se_tax: dec!(7500.00),
                total_tax: dec!(25000.00),
                required_payment: dec!(4000.00),
            })
        );
    }

    #[tokio::test]
    async fn update_not_found() {
        let repo = setup_test_db().await;
        seed(&repo).await;

        let mut created = TaxRepository::create::<TaxEstimate>(&repo, minimal_input())
            .await
            .expect("create");
        created.id = 99999;

        let result = TaxRepository::update(&repo, &created).await;
        assert!(matches!(result, Err(RepositoryError::NotFound)));
    }

    #[tokio::test]
    async fn delete_estimate() {
        let repo = setup_test_db().await;
        seed(&repo).await;

        let created = TaxRepository::create::<TaxEstimate>(&repo, minimal_input())
            .await
            .expect("create");
        let id = created.id;

        TaxRepository::delete::<TaxEstimate>(&repo, &id)
            .await
            .expect("delete");

        let result = repo.get::<TaxEstimate>(&id).await;
        assert!(matches!(result, Err(RepositoryError::NotFound)));
    }

    #[tokio::test]
    async fn delete_not_found() {
        let repo = setup_test_db().await;
        let result = TaxRepository::delete::<TaxEstimate>(&repo, &99999).await;
        assert!(matches!(result, Err(RepositoryError::NotFound)));
    }

    #[tokio::test]
    async fn list_estimates() {
        let repo = setup_test_db().await;
        seed(&repo).await;

        let est_8888 = minimal_input();
        let est_8887 = TaxEstimateInput {
            tax_year: 8887,
            ..minimal_input()
        };

        let first = TaxRepository::create::<TaxEstimate>(&repo, est_8888.clone())
            .await
            .expect("create");
        let second = TaxRepository::create::<TaxEstimate>(&repo, est_8888)
            .await
            .expect("upsert");
        assert_eq!(
            first.id, second.id,
            "same tax year and filing status should update the existing row"
        );
        TaxRepository::create::<TaxEstimate>(&repo, est_8887)
            .await
            .expect("create");

        let all = repo
            .list::<TaxEstimate>(&TaxEstimateFilter { tax_year: None })
            .await
            .expect("list all");
        assert_eq!(all.len(), 2);

        let for_8888 = repo
            .list::<TaxEstimate>(&TaxEstimateFilter {
                tax_year: Some(8888),
            })
            .await
            .expect("list 8888");
        assert_eq!(for_8888.len(), 1);
        assert!(for_8888.iter().all(|e| e.input.tax_year == 8888));

        let for_8887 = repo
            .list::<TaxEstimate>(&TaxEstimateFilter {
                tax_year: Some(8887),
            })
            .await
            .expect("list 8887");
        assert_eq!(for_8887.len(), 1);
        assert_eq!(for_8887[0].input.tax_year, 8887);

        let for_7777 = repo
            .list::<TaxEstimate>(&TaxEstimateFilter {
                tax_year: Some(7777),
            })
            .await
            .expect("list 7777");
        assert!(for_7777.is_empty());
    }
}
