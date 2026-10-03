use async_trait::async_trait;
use chrono::Utc;
use tax_core::{
    FilingStatus, Qbi, RepositoryError, StandardDeduction, TaxBracket, TaxEstimate,
    TaxEstimateInput, TaxRepository, TaxYearConfig,
};

use super::SqliteRepository;
use super::rows::{
    EstimateRow, FilingStatusDataRow, FilingStatusRow, QbiRow, StandardDeductionRow, TaxBracketRow,
    TaxYearConfigRow, parse_filing_status_code,
};
use crate::decimal::decimal_to_f64;

/// Expand to a fixed `SELECT` over `tax_estimate` joined to `filing_status`,
/// appending `$tail`. Produces a compile-time `&'static str`, so estimate reads
/// never build SQL at runtime.
macro_rules! estimate_select {
    ($tail:literal) => {
        concat!(
            "SELECT te.id, te.tax_year, te.expected_agi, te.expected_deduction,
                    te.expected_qbi_deduction, te.expected_amt, te.expected_credits,
                    te.expected_other_taxes, te.expected_withholding, te.prior_year_tax,
                    te.se_income, te.expected_crp_payments, te.expected_wages,
                    te.calculated_se_tax, te.calculated_total_tax, te.calculated_required_payment,
                    te.created_at, te.updated_at, fs.status_code AS filing_status_code
             FROM tax_estimate te
             JOIN filing_status fs ON fs.id = te.filing_status_id ",
            $tail
        )
    };
}

/// Bind the eleven monetary `TaxEstimateInput` columns, in the column order
/// shared by the insert/update statements, onto an already-started query.
macro_rules! bind_estimate_input {
    ($query:expr, $input:expr) => {
        $query
            .bind(decimal_to_f64($input.expected_agi))
            .bind(decimal_to_f64($input.expected_deduction))
            .bind($input.expected_qbi_deduction.map(decimal_to_f64))
            .bind($input.expected_amt.map(decimal_to_f64))
            .bind($input.expected_credits.map(decimal_to_f64))
            .bind($input.expected_other_taxes.map(decimal_to_f64))
            .bind($input.expected_withholding.map(decimal_to_f64))
            .bind($input.prior_year_tax.map(decimal_to_f64))
            .bind($input.se_income.map(decimal_to_f64))
            .bind($input.expected_crp_payments.map(decimal_to_f64))
            .bind($input.expected_wages.map(decimal_to_f64))
    };
}

/// Upsert a single estimate and return every persisted column. The filing
/// status is known from the input, so `RETURNING` need not (and, across a
/// join, could not) resolve the status code.
const CREATE_ESTIMATE_SQL: &str = "INSERT INTO tax_estimate (
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
    RETURNING
        id, tax_year, expected_agi, expected_deduction,
        expected_qbi_deduction, expected_amt, expected_credits,
        expected_other_taxes, expected_withholding, prior_year_tax,
        se_income, expected_crp_payments, expected_wages,
        calculated_se_tax, calculated_total_tax, calculated_required_payment,
        created_at, updated_at";

/// Read the Form 8995 data for one estimate. The LEFT JOIN repeats the `qbi`
/// columns once per Line 1 row, or once with NULL business columns when the
/// estimate has no Line 1 rows.
const SELECT_QBI_SQL: &str = "SELECT q.tax_estimate_id, q.qbi_loss_carryforward,
            q.reit_ptp_income, q.reit_ptp_loss_carryforward,
            q.taxable_income_before_qbi, q.net_capital_gain,
            q.calculated_qbi_deduction, q.calculated_qbi_loss_carryforward,
            q.calculated_reit_ptp_loss_carryforward,
            b.line_number, b.business_name, b.taxpayer_id, b.qualified_business_income
     FROM qbi q
     LEFT JOIN qbi_business b ON b.tax_estimate_id = q.tax_estimate_id
     WHERE q.tax_estimate_id = ?
     ORDER BY b.line_number";

/// Insert or update the `qbi` row for one estimate.
///
/// The values come from a `SELECT` guarded by `WHERE EXISTS`, so the statement
/// affects no rows when the estimate does not exist. SQLite also requires a
/// `WHERE` clause on the `SELECT` for `ON CONFLICT` to be parsed after it.
const UPSERT_QBI_SQL: &str = "INSERT INTO qbi (
        tax_estimate_id, qbi_loss_carryforward, reit_ptp_income,
        reit_ptp_loss_carryforward, taxable_income_before_qbi, net_capital_gain,
        calculated_qbi_deduction, calculated_qbi_loss_carryforward,
        calculated_reit_ptp_loss_carryforward
    )
    SELECT ?, ?, ?, ?, ?, ?, ?, ?, ?
    WHERE EXISTS (SELECT 1 FROM tax_estimate WHERE id = ?)
    ON CONFLICT (tax_estimate_id) DO UPDATE SET
        qbi_loss_carryforward = excluded.qbi_loss_carryforward,
        reit_ptp_income = excluded.reit_ptp_income,
        reit_ptp_loss_carryforward = excluded.reit_ptp_loss_carryforward,
        taxable_income_before_qbi = excluded.taxable_income_before_qbi,
        net_capital_gain = excluded.net_capital_gain,
        calculated_qbi_deduction = excluded.calculated_qbi_deduction,
        calculated_qbi_loss_carryforward = excluded.calculated_qbi_loss_carryforward,
        calculated_reit_ptp_loss_carryforward = excluded.calculated_reit_ptp_loss_carryforward";

fn db_err(error: sqlx::Error) -> RepositoryError {
    RepositoryError::Database(error.into())
}

#[async_trait]
impl TaxRepository for SqliteRepository {
    async fn get_tax_year_config(
        &self,
        year: i32,
    ) -> Result<TaxYearConfig, RepositoryError> {
        let row = sqlx::query_as::<_, TaxYearConfigRow>(
            "SELECT tax_year, ss_wage_max, ss_tax_rate, medicare_tax_rate,
                    se_tax_deductible_percentage, se_deduction_factor,
                    required_payment_threshold, min_se_threshold
             FROM tax_year_config WHERE tax_year = ?",
        )
        .bind(year)
        .fetch_optional(self.pool())
        .await
        .map_err(db_err)?
        .ok_or(RepositoryError::NotFound)?;

        TaxYearConfig::try_from(row)
    }

    async fn list_tax_years(&self) -> Result<Vec<i32>, RepositoryError> {
        sqlx::query_scalar::<_, i32>("SELECT tax_year FROM tax_year_config ORDER BY tax_year DESC")
            .fetch_all(self.pool())
            .await
            .map_err(db_err)
    }

    async fn get_filing_status(
        &self,
        id: i32,
    ) -> Result<FilingStatus, RepositoryError> {
        let row = sqlx::query_as::<_, FilingStatusRow>(
            "SELECT id, status_code, status_name FROM filing_status WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(self.pool())
        .await
        .map_err(db_err)?
        .ok_or(RepositoryError::NotFound)?;

        FilingStatus::try_from(row)
    }

    async fn get_filing_status_by_code(
        &self,
        code: &str,
    ) -> Result<FilingStatus, RepositoryError> {
        let row = sqlx::query_as::<_, FilingStatusRow>(
            "SELECT id, status_code, status_name FROM filing_status WHERE status_code = ?",
        )
        .bind(code)
        .fetch_optional(self.pool())
        .await
        .map_err(db_err)?
        .ok_or(RepositoryError::NotFound)?;

        FilingStatus::try_from(row)
    }

    async fn list_filing_statuses(&self) -> Result<Vec<FilingStatus>, RepositoryError> {
        sqlx::query_as::<_, FilingStatusRow>(
            "SELECT id, status_code, status_name FROM filing_status ORDER BY id",
        )
        .fetch_all(self.pool())
        .await
        .map_err(db_err)?
        .into_iter()
        .map(FilingStatus::try_from)
        .collect()
    }

    async fn get_standard_deduction(
        &self,
        tax_year: i32,
        filing_status_id: i32,
    ) -> Result<StandardDeduction, RepositoryError> {
        let row = sqlx::query_as::<_, StandardDeductionRow>(
            "SELECT tax_year, filing_status_id, amount
             FROM standard_deductions
             WHERE tax_year = ? AND filing_status_id = ?",
        )
        .bind(tax_year)
        .bind(filing_status_id)
        .fetch_optional(self.pool())
        .await
        .map_err(db_err)?
        .ok_or(RepositoryError::NotFound)?;

        StandardDeduction::try_from(row)
    }

    async fn get_filing_status_data(
        &self,
        year: i32,
    ) -> Result<Vec<(FilingStatus, StandardDeduction, Vec<TaxBracket>)>, RepositoryError> {
        let rows = sqlx::query_as::<_, FilingStatusDataRow>(
            "SELECT
                fs.id          AS status_id,
                fs.status_code,
                fs.status_name,
                sd.amount      AS deduction_amount,
                tb.min_income,
                tb.max_income,
                tb.tax_rate,
                tb.base_tax
             FROM filing_status fs
             JOIN standard_deductions sd
                 ON sd.filing_status_id = fs.id AND sd.tax_year = ?
             LEFT JOIN tax_brackets tb
                 ON tb.filing_status_id = fs.id AND tb.tax_year = ?
             ORDER BY fs.id, tb.min_income",
        )
        .bind(year)
        .bind(year)
        .fetch_all(self.pool())
        .await
        .map_err(db_err)?;

        let mut result: Vec<(FilingStatus, StandardDeduction, Vec<TaxBracket>)> = Vec::new();
        let mut current: Option<(FilingStatus, StandardDeduction, Vec<TaxBracket>)> = None;

        for row in rows {
            let is_new_group = current
                .as_ref()
                .is_none_or(|(fs, _, _)| fs.id != row.status_id);

            if is_new_group {
                if let Some(group) = current.take() {
                    result.push(group);
                }

                let filing_status = FilingStatus {
                    id: row.status_id,
                    status_code: parse_filing_status_code(&row.status_code)?,
                    status_name: row.status_name,
                };
                let deduction = StandardDeduction {
                    tax_year: year,
                    filing_status_id: row.status_id,
                    amount: row.deduction_amount,
                };
                current = Some((filing_status, deduction, Vec::new()));
            }

            // LEFT JOIN: bracket columns are NULL (so `min_income` is `None`)
            // when a filing status has no brackets for the year.
            if let Some(min_income) = row.min_income {
                let bracket = TaxBracket {
                    tax_year: year,
                    filing_status_id: row.status_id,
                    min_income,
                    max_income: row.max_income,
                    tax_rate: row.tax_rate,
                    base_tax: row.base_tax,
                };
                if let Some((_, _, brackets)) = &mut current {
                    brackets.push(bracket);
                }
            }
        }

        if let Some(group) = current.take() {
            result.push(group);
        }

        Ok(result)
    }

    async fn get_tax_brackets(
        &self,
        tax_year: i32,
        filing_status_id: i32,
    ) -> Result<Vec<TaxBracket>, RepositoryError> {
        sqlx::query_as::<_, TaxBracketRow>(
            "SELECT tax_year, filing_status_id, min_income, max_income, tax_rate, base_tax
             FROM tax_brackets
             WHERE tax_year = ? AND filing_status_id = ?
             ORDER BY min_income",
        )
        .bind(tax_year)
        .bind(filing_status_id)
        .fetch_all(self.pool())
        .await
        .map_err(db_err)?
        .into_iter()
        .map(TaxBracket::try_from)
        .collect()
    }

    async fn insert_tax_bracket(
        &self,
        bracket: &TaxBracket,
    ) -> Result<(), RepositoryError> {
        sqlx::query(
            "INSERT INTO tax_brackets (tax_year, filing_status_id, min_income, max_income, tax_rate, base_tax)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(bracket.tax_year)
        .bind(bracket.filing_status_id)
        .bind(decimal_to_f64(bracket.min_income))
        .bind(bracket.max_income.map(decimal_to_f64))
        .bind(decimal_to_f64(bracket.tax_rate))
        .bind(decimal_to_f64(bracket.base_tax))
        .execute(self.pool())
        .await
        .map_err(db_err)?;
        Ok(())
    }

    async fn delete_tax_brackets(
        &self,
        tax_year: i32,
        filing_status_id: i32,
    ) -> Result<(), RepositoryError> {
        sqlx::query("DELETE FROM tax_brackets WHERE tax_year = ? AND filing_status_id = ?")
            .bind(tax_year)
            .bind(filing_status_id)
            .execute(self.pool())
            .await
            .map_err(db_err)?;
        Ok(())
    }

    async fn create_estimate(
        &self,
        estimate: TaxEstimateInput,
    ) -> Result<TaxEstimate, RepositoryError> {
        let now = Utc::now();
        let filing_status_id = self
            .filing_status_id_for_code(estimate.filing_status)
            .await?;

        let query = sqlx::query(CREATE_ESTIMATE_SQL)
            .bind(estimate.tax_year)
            .bind(filing_status_id);

        // `RETURNING` hands back the persisted row in one round-trip; the
        // filing-status code is already known, so no follow-up read is needed.
        let returned = bind_estimate_input!(query, estimate)
            .bind(now)
            .bind(now)
            .fetch_one(self.pool())
            .await
            .map_err(db_err)?;

        let row = EstimateRow::from_row_with_code(
            &returned,
            estimate.filing_status.as_str().to_string(),
        )?;
        TaxEstimate::try_from(row)
    }

    async fn get_estimate(
        &self,
        id: i64,
    ) -> Result<TaxEstimate, RepositoryError> {
        let row = sqlx::query_as::<_, EstimateRow>(estimate_select!("WHERE te.id = ?"))
            .bind(id)
            .fetch_optional(self.pool())
            .await
            .map_err(db_err)?
            .ok_or(RepositoryError::NotFound)?;

        TaxEstimate::try_from(row)
    }

    async fn update_estimate(
        &self,
        estimate: &TaxEstimate,
    ) -> Result<(), RepositoryError> {
        let now = Utc::now();
        let filing_status_id = self
            .filing_status_id_for_code(estimate.input.filing_status)
            .await?;

        let (calculated_se_tax, calculated_total_tax, calculated_required_payment) =
            match &estimate.computed {
                Some(computed) => (
                    Some(decimal_to_f64(computed.se_tax)),
                    Some(decimal_to_f64(computed.total_tax)),
                    Some(decimal_to_f64(computed.required_payment)),
                ),
                None => (None, None, None),
            };

        let query = sqlx::query(
            "UPDATE tax_estimate SET
                tax_year = ?, filing_status_id = ?, expected_agi = ?, expected_deduction = ?,
                expected_qbi_deduction = ?, expected_amt = ?, expected_credits = ?,
                expected_other_taxes = ?, expected_withholding = ?, prior_year_tax = ?,
                se_income = ?, expected_crp_payments = ?, expected_wages = ?,
                calculated_se_tax = ?, calculated_total_tax = ?, calculated_required_payment = ?,
                updated_at = ?
             WHERE id = ?",
        )
        .bind(estimate.input.tax_year)
        .bind(filing_status_id);

        let result = bind_estimate_input!(query, estimate.input)
            .bind(calculated_se_tax)
            .bind(calculated_total_tax)
            .bind(calculated_required_payment)
            .bind(now)
            .bind(estimate.id)
            .execute(self.pool())
            .await
            .map_err(db_err)?;

        if result.rows_affected() == 0 {
            return Err(RepositoryError::NotFound);
        }
        Ok(())
    }

    async fn delete_estimate(
        &self,
        id: i64,
    ) -> Result<(), RepositoryError> {
        let result = sqlx::query("DELETE FROM tax_estimate WHERE id = ?")
            .bind(id)
            .execute(self.pool())
            .await
            .map_err(db_err)?;

        if result.rows_affected() == 0 {
            return Err(RepositoryError::NotFound);
        }
        Ok(())
    }

    async fn list_estimates(
        &self,
        tax_year: Option<i32>,
    ) -> Result<Vec<TaxEstimate>, RepositoryError> {
        let rows = match tax_year {
            Some(year) => {
                sqlx::query_as::<_, EstimateRow>(estimate_select!(
                    "WHERE te.tax_year = ? ORDER BY te.updated_at DESC"
                ))
                .bind(year)
                .fetch_all(self.pool())
                .await
            }
            None => {
                sqlx::query_as::<_, EstimateRow>(estimate_select!("ORDER BY te.updated_at DESC"))
                    .fetch_all(self.pool())
                    .await
            }
        }
        .map_err(db_err)?;

        rows.into_iter().map(TaxEstimate::try_from).collect()
    }

    async fn get_qbi(
        &self,
        estimate_id: i64,
    ) -> Result<Option<Qbi>, RepositoryError> {
        let rows = sqlx::query_as::<_, QbiRow>(SELECT_QBI_SQL)
            .bind(estimate_id)
            .fetch_all(self.pool())
            .await
            .map_err(db_err)?;

        QbiRow::into_qbi(rows)
    }

    async fn save_qbi(
        &self,
        qbi: &Qbi,
    ) -> Result<(), RepositoryError> {
        let (
            calculated_qbi_deduction,
            calculated_qbi_loss_carryforward,
            calculated_reit_ptp_loss_carryforward,
        ) = match &qbi.computed {
            Some(computed) => (
                Some(decimal_to_f64(computed.qbi_deduction)),
                Some(decimal_to_f64(computed.total_qbi_loss_carryforward)),
                Some(decimal_to_f64(computed.total_reit_ptp_loss_carryforward)),
            ),
            None => (None, None, None),
        };

        let mut tx = self.pool().begin().await.map_err(db_err)?;

        let result = sqlx::query(UPSERT_QBI_SQL)
            .bind(qbi.tax_estimate_id)
            .bind(decimal_to_f64(qbi.input.qbi_loss_carryforward))
            .bind(decimal_to_f64(qbi.input.reit_ptp_income))
            .bind(decimal_to_f64(qbi.input.reit_ptp_loss_carryforward))
            .bind(qbi.input.taxable_income_before_qbi.map(decimal_to_f64))
            .bind(decimal_to_f64(qbi.input.net_capital_gain))
            .bind(calculated_qbi_deduction)
            .bind(calculated_qbi_loss_carryforward)
            .bind(calculated_reit_ptp_loss_carryforward)
            .bind(qbi.tax_estimate_id)
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
        if result.rows_affected() == 0 {
            // No estimate has this id. Dropping `tx` rolls the transaction back.
            return Err(RepositoryError::NotFound);
        }

        // Replace the Line 1 rows as a set, so removed and reordered rows need
        // no separate handling.
        sqlx::query("DELETE FROM qbi_business WHERE tax_estimate_id = ?")
            .bind(qbi.tax_estimate_id)
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
        for (line_number, business) in (1_i64..).zip(&qbi.input.businesses) {
            sqlx::query(
                "INSERT INTO qbi_business (
                    tax_estimate_id, line_number, business_name, taxpayer_id,
                    qualified_business_income
                 ) VALUES (?, ?, ?, ?, ?)",
            )
            .bind(qbi.tax_estimate_id)
            .bind(line_number)
            .bind(business.name.as_str())
            .bind(business.taxpayer_id.as_str())
            .bind(decimal_to_f64(business.qualified_business_income))
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
        }

        tx.commit().await.map_err(db_err)
    }

    async fn delete_qbi(
        &self,
        estimate_id: i64,
    ) -> Result<(), RepositoryError> {
        // The qbi_business rows go with it through ON DELETE CASCADE.
        let result = sqlx::query("DELETE FROM qbi WHERE tax_estimate_id = ?")
            .bind(estimate_id)
            .execute(self.pool())
            .await
            .map_err(db_err)?;

        if result.rows_affected() == 0 {
            return Err(RepositoryError::NotFound);
        }
        Ok(())
    }
}
