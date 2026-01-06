use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::{sqlite::SqlitePool, FromRow};
use tax_core::{
    EstimatedTaxCalculation, FilingStatus, FilingStatusCode, NewEstimatedTaxCalculation,
    RepositoryError, StandardDeduction, TaxBracket, TaxRepository, TaxYearConfig,
};

pub struct SqliteRepository {
    pool: SqlitePool,
}

impl SqliteRepository {
    pub async fn new(database_url: &str) -> Result<Self, RepositoryError> {
        let pool = SqlitePool::connect(database_url)
            .await
            .map_err(|e| RepositoryError::Connection(e.to_string()))?;
        Ok(Self { pool })
    }

    pub async fn new_with_pool(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn run_migrations(&self) -> Result<(), RepositoryError> {
        sqlx::migrate!("./migrations")
            .run(&self.pool)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?;
        Ok(())
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}

#[derive(FromRow)]
struct TaxYearConfigRow {
    tax_year: i32,
    ss_wage_max: String,
    ss_tax_rate: String,
    medicare_tax_rate: String,
    se_tax_deductible_percentage: String,
    se_deduction_factor: String,
    required_payment_threshold: String,
}

impl TryFrom<TaxYearConfigRow> for TaxYearConfig {
    type Error = RepositoryError;

    fn try_from(row: TaxYearConfigRow) -> Result<Self, Self::Error> {
        Ok(TaxYearConfig {
            tax_year: row.tax_year,
            ss_wage_max: parse_decimal(&row.ss_wage_max)?,
            ss_tax_rate: parse_decimal(&row.ss_tax_rate)?,
            medicare_tax_rate: parse_decimal(&row.medicare_tax_rate)?,
            se_tax_deductible_percentage: parse_decimal(&row.se_tax_deductible_percentage)?,
            se_deduction_factor: parse_decimal(&row.se_deduction_factor)?,
            required_payment_threshold: parse_decimal(&row.required_payment_threshold)?,
        })
    }
}

#[derive(FromRow)]
struct FilingStatusRow {
    id: i32,
    status_code: String,
    status_name: String,
}

impl TryFrom<FilingStatusRow> for FilingStatus {
    type Error = RepositoryError;

    fn try_from(row: FilingStatusRow) -> Result<Self, Self::Error> {
        let status_code = FilingStatusCode::from_str(&row.status_code)
            .ok_or_else(|| RepositoryError::Database(format!("Invalid status code: {}", row.status_code)))?;
        Ok(FilingStatus {
            id: row.id,
            status_code,
            status_name: row.status_name,
        })
    }
}

#[derive(FromRow)]
struct StandardDeductionRow {
    tax_year: i32,
    filing_status_id: i32,
    amount: String,
}

impl TryFrom<StandardDeductionRow> for StandardDeduction {
    type Error = RepositoryError;

    fn try_from(row: StandardDeductionRow) -> Result<Self, Self::Error> {
        Ok(StandardDeduction {
            tax_year: row.tax_year,
            filing_status_id: row.filing_status_id,
            amount: parse_decimal(&row.amount)?,
        })
    }
}

#[derive(FromRow)]
struct TaxBracketRow {
    tax_year: i32,
    filing_status_id: i32,
    min_income: String,
    max_income: Option<String>,
    tax_rate: String,
    base_tax: String,
}

impl TryFrom<TaxBracketRow> for TaxBracket {
    type Error = RepositoryError;

    fn try_from(row: TaxBracketRow) -> Result<Self, Self::Error> {
        Ok(TaxBracket {
            tax_year: row.tax_year,
            filing_status_id: row.filing_status_id,
            min_income: parse_decimal(&row.min_income)?,
            max_income: row.max_income.as_ref().map(|s| parse_decimal(s)).transpose()?,
            tax_rate: parse_decimal(&row.tax_rate)?,
            base_tax: parse_decimal(&row.base_tax)?,
        })
    }
}

#[derive(FromRow)]
struct EstimatedTaxCalculationRow {
    id: i64,
    tax_year: i32,
    filing_status_id: i32,
    expected_agi: String,
    expected_deduction: String,
    expected_qbi_deduction: Option<String>,
    expected_amt: Option<String>,
    expected_credits: Option<String>,
    expected_other_taxes: Option<String>,
    prior_year_tax: Option<String>,
    expected_withholding: Option<String>,
    se_income: Option<String>,
    expected_crp_payments: Option<String>,
    expected_wages: Option<String>,
    calculated_se_tax: Option<String>,
    calculated_total_tax: Option<String>,
    calculated_required_payment: Option<String>,
    created_at: String,
    updated_at: String,
}

impl TryFrom<EstimatedTaxCalculationRow> for EstimatedTaxCalculation {
    type Error = RepositoryError;

    fn try_from(row: EstimatedTaxCalculationRow) -> Result<Self, Self::Error> {
        Ok(EstimatedTaxCalculation {
            id: row.id,
            tax_year: row.tax_year,
            filing_status_id: row.filing_status_id,
            expected_agi: parse_decimal(&row.expected_agi)?,
            expected_deduction: parse_decimal(&row.expected_deduction)?,
            expected_qbi_deduction: parse_optional_decimal(&row.expected_qbi_deduction)?,
            expected_amt: parse_optional_decimal(&row.expected_amt)?,
            expected_credits: parse_optional_decimal(&row.expected_credits)?,
            expected_other_taxes: parse_optional_decimal(&row.expected_other_taxes)?,
            prior_year_tax: parse_optional_decimal(&row.prior_year_tax)?,
            expected_withholding: parse_optional_decimal(&row.expected_withholding)?,
            se_income: parse_optional_decimal(&row.se_income)?,
            expected_crp_payments: parse_optional_decimal(&row.expected_crp_payments)?,
            expected_wages: parse_optional_decimal(&row.expected_wages)?,
            calculated_se_tax: parse_optional_decimal(&row.calculated_se_tax)?,
            calculated_total_tax: parse_optional_decimal(&row.calculated_total_tax)?,
            calculated_required_payment: parse_optional_decimal(&row.calculated_required_payment)?,
            created_at: parse_datetime(&row.created_at)?,
            updated_at: parse_datetime(&row.updated_at)?,
        })
    }
}

fn parse_decimal(s: &str) -> Result<Decimal, RepositoryError> {
    s.parse::<Decimal>()
        .map_err(|e| RepositoryError::Database(format!("Failed to parse decimal '{}': {}", s, e)))
}

fn parse_optional_decimal(s: &Option<String>) -> Result<Option<Decimal>, RepositoryError> {
    s.as_ref().map(|s| parse_decimal(s)).transpose()
}

fn parse_datetime(s: &str) -> Result<DateTime<Utc>, RepositoryError> {
    // SQLite stores timestamps in various formats, try common ones
    chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")
        .or_else(|_| chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S"))
        .or_else(|_| chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S%.f"))
        .map(|naive| naive.and_utc())
        .map_err(|e| RepositoryError::Database(format!("Failed to parse datetime '{}': {}", s, e)))
}

#[async_trait]
impl TaxRepository for SqliteRepository {
    async fn get_tax_year_config(&self, year: i32) -> Result<TaxYearConfig, RepositoryError> {
        let row: TaxYearConfigRow = sqlx::query_as(
            "SELECT tax_year, ss_wage_max, ss_tax_rate, medicare_tax_rate,
                    se_tax_deductible_percentage, se_deduction_factor, required_payment_threshold
             FROM tax_year_config WHERE tax_year = ?"
        )
        .bind(year)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| RepositoryError::Database(e.to_string()))?
        .ok_or(RepositoryError::NotFound)?;

        row.try_into()
    }

    async fn list_tax_years(&self) -> Result<Vec<i32>, RepositoryError> {
        let rows: Vec<(i32,)> = sqlx::query_as("SELECT tax_year FROM tax_year_config ORDER BY tax_year DESC")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?;

        Ok(rows.into_iter().map(|(year,)| year).collect())
    }

    async fn get_filing_status(&self, id: i32) -> Result<FilingStatus, RepositoryError> {
        let row: FilingStatusRow = sqlx::query_as(
            "SELECT id, status_code, status_name FROM filing_status WHERE id = ?"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| RepositoryError::Database(e.to_string()))?
        .ok_or(RepositoryError::NotFound)?;

        row.try_into()
    }

    async fn list_filing_statuses(&self) -> Result<Vec<FilingStatus>, RepositoryError> {
        let rows: Vec<FilingStatusRow> = sqlx::query_as(
            "SELECT id, status_code, status_name FROM filing_status ORDER BY id"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RepositoryError::Database(e.to_string()))?;

        rows.into_iter().map(|r| r.try_into()).collect()
    }

    async fn get_standard_deduction(
        &self,
        tax_year: i32,
        filing_status_id: i32,
    ) -> Result<StandardDeduction, RepositoryError> {
        let row: StandardDeductionRow = sqlx::query_as(
            "SELECT tax_year, filing_status_id, amount
             FROM standard_deductions
             WHERE tax_year = ? AND filing_status_id = ?"
        )
        .bind(tax_year)
        .bind(filing_status_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| RepositoryError::Database(e.to_string()))?
        .ok_or(RepositoryError::NotFound)?;

        row.try_into()
    }

    async fn get_tax_brackets(
        &self,
        tax_year: i32,
        filing_status_id: i32,
    ) -> Result<Vec<TaxBracket>, RepositoryError> {
        let rows: Vec<TaxBracketRow> = sqlx::query_as(
            "SELECT tax_year, filing_status_id, min_income, max_income, tax_rate, base_tax
             FROM tax_brackets
             WHERE tax_year = ? AND filing_status_id = ?
             ORDER BY min_income"
        )
        .bind(tax_year)
        .bind(filing_status_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RepositoryError::Database(e.to_string()))?;

        rows.into_iter().map(|r| r.try_into()).collect()
    }

    async fn create_calculation(
        &self,
        calc: NewEstimatedTaxCalculation,
    ) -> Result<EstimatedTaxCalculation, RepositoryError> {
        let now = Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();

        let result = sqlx::query(
            "INSERT INTO estimated_tax_calculation (
                tax_year, filing_status_id, expected_agi, expected_deduction,
                expected_qbi_deduction, expected_amt, expected_credits,
                expected_other_taxes, prior_year_tax, expected_withholding,
                se_income, expected_crp_payments, expected_wages,
                created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(calc.tax_year)
        .bind(calc.filing_status_id)
        .bind(calc.expected_agi.to_string())
        .bind(calc.expected_deduction.to_string())
        .bind(calc.expected_qbi_deduction.map(|d| d.to_string()))
        .bind(calc.expected_amt.map(|d| d.to_string()))
        .bind(calc.expected_credits.map(|d| d.to_string()))
        .bind(calc.expected_other_taxes.map(|d| d.to_string()))
        .bind(calc.prior_year_tax.map(|d| d.to_string()))
        .bind(calc.expected_withholding.map(|d| d.to_string()))
        .bind(calc.se_income.map(|d| d.to_string()))
        .bind(calc.expected_crp_payments.map(|d| d.to_string()))
        .bind(calc.expected_wages.map(|d| d.to_string()))
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await
        .map_err(|e| RepositoryError::Database(e.to_string()))?;

        let id = result.last_insert_rowid();
        self.get_calculation(id).await
    }

    async fn get_calculation(&self, id: i64) -> Result<EstimatedTaxCalculation, RepositoryError> {
        let row: EstimatedTaxCalculationRow = sqlx::query_as(
            "SELECT id, tax_year, filing_status_id, expected_agi, expected_deduction,
                    expected_qbi_deduction, expected_amt, expected_credits,
                    expected_other_taxes, prior_year_tax, expected_withholding,
                    se_income, expected_crp_payments, expected_wages,
                    calculated_se_tax, calculated_total_tax, calculated_required_payment,
                    created_at, updated_at
             FROM estimated_tax_calculation WHERE id = ?"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| RepositoryError::Database(e.to_string()))?
        .ok_or(RepositoryError::NotFound)?;

        row.try_into()
    }

    async fn update_calculation(
        &self,
        calc: &EstimatedTaxCalculation,
    ) -> Result<(), RepositoryError> {
        let now = Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();

        let result = sqlx::query(
            "UPDATE estimated_tax_calculation SET
                tax_year = ?, filing_status_id = ?, expected_agi = ?, expected_deduction = ?,
                expected_qbi_deduction = ?, expected_amt = ?, expected_credits = ?,
                expected_other_taxes = ?, prior_year_tax = ?, expected_withholding = ?,
                se_income = ?, expected_crp_payments = ?, expected_wages = ?,
                calculated_se_tax = ?, calculated_total_tax = ?, calculated_required_payment = ?,
                updated_at = ?
             WHERE id = ?"
        )
        .bind(calc.tax_year)
        .bind(calc.filing_status_id)
        .bind(calc.expected_agi.to_string())
        .bind(calc.expected_deduction.to_string())
        .bind(calc.expected_qbi_deduction.map(|d| d.to_string()))
        .bind(calc.expected_amt.map(|d| d.to_string()))
        .bind(calc.expected_credits.map(|d| d.to_string()))
        .bind(calc.expected_other_taxes.map(|d| d.to_string()))
        .bind(calc.prior_year_tax.map(|d| d.to_string()))
        .bind(calc.expected_withholding.map(|d| d.to_string()))
        .bind(calc.se_income.map(|d| d.to_string()))
        .bind(calc.expected_crp_payments.map(|d| d.to_string()))
        .bind(calc.expected_wages.map(|d| d.to_string()))
        .bind(calc.calculated_se_tax.map(|d| d.to_string()))
        .bind(calc.calculated_total_tax.map(|d| d.to_string()))
        .bind(calc.calculated_required_payment.map(|d| d.to_string()))
        .bind(&now)
        .bind(calc.id)
        .execute(&self.pool)
        .await
        .map_err(|e| RepositoryError::Database(e.to_string()))?;

        if result.rows_affected() == 0 {
            return Err(RepositoryError::NotFound);
        }

        Ok(())
    }

    async fn delete_calculation(&self, id: i64) -> Result<(), RepositoryError> {
        let result = sqlx::query("DELETE FROM estimated_tax_calculation WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?;

        if result.rows_affected() == 0 {
            return Err(RepositoryError::NotFound);
        }

        Ok(())
    }

    async fn list_calculations(
        &self,
        tax_year: Option<i32>,
    ) -> Result<Vec<EstimatedTaxCalculation>, RepositoryError> {
        let rows: Vec<EstimatedTaxCalculationRow> = match tax_year {
            Some(year) => {
                sqlx::query_as(
                    "SELECT id, tax_year, filing_status_id, expected_agi, expected_deduction,
                            expected_qbi_deduction, expected_amt, expected_credits,
                            expected_other_taxes, prior_year_tax, expected_withholding,
                            se_income, expected_crp_payments, expected_wages,
                            calculated_se_tax, calculated_total_tax, calculated_required_payment,
                            created_at, updated_at
                     FROM estimated_tax_calculation WHERE tax_year = ? ORDER BY updated_at DESC"
                )
                .bind(year)
                .fetch_all(&self.pool)
                .await
            }
            None => {
                sqlx::query_as(
                    "SELECT id, tax_year, filing_status_id, expected_agi, expected_deduction,
                            expected_qbi_deduction, expected_amt, expected_credits,
                            expected_other_taxes, prior_year_tax, expected_withholding,
                            se_income, expected_crp_payments, expected_wages,
                            calculated_se_tax, calculated_total_tax, calculated_required_payment,
                            created_at, updated_at
                     FROM estimated_tax_calculation ORDER BY updated_at DESC"
                )
                .fetch_all(&self.pool)
                .await
            }
        }
        .map_err(|e| RepositoryError::Database(e.to_string()))?;

        rows.into_iter().map(|r| r.try_into()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn setup_test_db() -> SqliteRepository {
        let pool = SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .expect("Failed to create in-memory database");

        let repo = SqliteRepository::new_with_pool(pool).await;
        repo.run_migrations().await.expect("Failed to run migrations");
        repo
    }

    #[tokio::test]
    async fn test_get_tax_year_config() {
        let repo = setup_test_db().await;

        let config = repo.get_tax_year_config(2025).await.expect("Should find 2025 config");

        assert_eq!(config.tax_year, 2025);
        assert_eq!(config.ss_wage_max, dec!(176100.00));
        assert_eq!(config.ss_tax_rate, dec!(0.124));
        assert_eq!(config.medicare_tax_rate, dec!(0.029));
        assert_eq!(config.se_tax_deductible_percentage, dec!(0.9235));
        assert_eq!(config.se_deduction_factor, dec!(0.50));
        assert_eq!(config.required_payment_threshold, dec!(1000.00));
    }

    #[tokio::test]
    async fn test_get_tax_year_config_not_found() {
        let repo = setup_test_db().await;

        let result = repo.get_tax_year_config(1999).await;

        assert!(matches!(result, Err(RepositoryError::NotFound)));
    }

    #[tokio::test]
    async fn test_list_tax_years() {
        let repo = setup_test_db().await;

        let years = repo.list_tax_years().await.expect("Should list tax years");

        assert!(years.contains(&2025));
    }

    #[tokio::test]
    async fn test_list_filing_statuses() {
        let repo = setup_test_db().await;

        let statuses = repo.list_filing_statuses().await.expect("Should list filing statuses");

        assert_eq!(statuses.len(), 5);
        assert!(statuses.iter().any(|s| s.status_code == FilingStatusCode::Single));
        assert!(statuses.iter().any(|s| s.status_code == FilingStatusCode::MarriedFilingJointly));
        assert!(statuses.iter().any(|s| s.status_code == FilingStatusCode::MarriedFilingSeparately));
        assert!(statuses.iter().any(|s| s.status_code == FilingStatusCode::HeadOfHousehold));
        assert!(statuses.iter().any(|s| s.status_code == FilingStatusCode::QualifyingSurvivingSpouse));
    }

    #[tokio::test]
    async fn test_get_filing_status() {
        let repo = setup_test_db().await;

        let status = repo.get_filing_status(1).await.expect("Should find filing status 1");

        assert_eq!(status.status_code, FilingStatusCode::Single);
        assert_eq!(status.status_name, "Single");
    }

    #[tokio::test]
    async fn test_get_standard_deduction() {
        let repo = setup_test_db().await;

        let deduction = repo.get_standard_deduction(2025, 1).await.expect("Should find standard deduction");

        assert_eq!(deduction.tax_year, 2025);
        assert_eq!(deduction.filing_status_id, 1);
        assert_eq!(deduction.amount, dec!(15000.00));
    }

    #[tokio::test]
    async fn test_get_tax_brackets() {
        let repo = setup_test_db().await;

        let brackets = repo.get_tax_brackets(2025, 1).await.expect("Should find tax brackets");

        assert_eq!(brackets.len(), 7);
        assert_eq!(brackets[0].tax_rate, dec!(0.10));
        assert_eq!(brackets[6].tax_rate, dec!(0.37));
        assert!(brackets[6].max_income.is_none()); // Top bracket has no max
    }

    #[tokio::test]
    async fn test_create_and_get_calculation() {
        let repo = setup_test_db().await;

        let new_calc = NewEstimatedTaxCalculation {
            tax_year: 2025,
            filing_status_id: 1,
            expected_agi: dec!(100000.00),
            expected_deduction: dec!(15000.00),
            expected_qbi_deduction: Some(dec!(5000.00)),
            expected_amt: None,
            expected_credits: Some(dec!(2000.00)),
            expected_other_taxes: None,
            prior_year_tax: Some(dec!(12000.00)),
            expected_withholding: Some(dec!(8000.00)),
            se_income: Some(dec!(50000.00)),
            expected_crp_payments: None,
            expected_wages: Some(dec!(50000.00)),
        };

        let created = repo.create_calculation(new_calc).await.expect("Should create calculation");

        assert!(created.id > 0);
        assert_eq!(created.tax_year, 2025);
        assert_eq!(created.expected_agi, dec!(100000.00));

        let fetched = repo.get_calculation(created.id).await.expect("Should fetch calculation");
        assert_eq!(fetched.id, created.id);
        assert_eq!(fetched.expected_agi, dec!(100000.00));
    }

    #[tokio::test]
    async fn test_update_calculation() {
        let repo = setup_test_db().await;

        let new_calc = NewEstimatedTaxCalculation {
            tax_year: 2025,
            filing_status_id: 1,
            expected_agi: dec!(100000.00),
            expected_deduction: dec!(15000.00),
            expected_qbi_deduction: None,
            expected_amt: None,
            expected_credits: None,
            expected_other_taxes: None,
            prior_year_tax: None,
            expected_withholding: None,
            se_income: None,
            expected_crp_payments: None,
            expected_wages: None,
        };

        let mut created = repo.create_calculation(new_calc).await.expect("Should create calculation");

        created.expected_agi = dec!(150000.00);
        created.calculated_total_tax = Some(dec!(25000.00));

        repo.update_calculation(&created).await.expect("Should update calculation");

        let fetched = repo.get_calculation(created.id).await.expect("Should fetch calculation");
        assert_eq!(fetched.expected_agi, dec!(150000.00));
        assert_eq!(fetched.calculated_total_tax, Some(dec!(25000.00)));
    }

    #[tokio::test]
    async fn test_delete_calculation() {
        let repo = setup_test_db().await;

        let new_calc = NewEstimatedTaxCalculation {
            tax_year: 2025,
            filing_status_id: 1,
            expected_agi: dec!(100000.00),
            expected_deduction: dec!(15000.00),
            expected_qbi_deduction: None,
            expected_amt: None,
            expected_credits: None,
            expected_other_taxes: None,
            prior_year_tax: None,
            expected_withholding: None,
            se_income: None,
            expected_crp_payments: None,
            expected_wages: None,
        };

        let created = repo.create_calculation(new_calc).await.expect("Should create calculation");
        let id = created.id;

        repo.delete_calculation(id).await.expect("Should delete calculation");

        let result = repo.get_calculation(id).await;
        assert!(matches!(result, Err(RepositoryError::NotFound)));
    }

    #[tokio::test]
    async fn test_list_calculations() {
        let repo = setup_test_db().await;

        let new_calc = NewEstimatedTaxCalculation {
            tax_year: 2025,
            filing_status_id: 1,
            expected_agi: dec!(100000.00),
            expected_deduction: dec!(15000.00),
            expected_qbi_deduction: None,
            expected_amt: None,
            expected_credits: None,
            expected_other_taxes: None,
            prior_year_tax: None,
            expected_withholding: None,
            se_income: None,
            expected_crp_payments: None,
            expected_wages: None,
        };

        repo.create_calculation(new_calc.clone()).await.expect("Should create calculation");
        repo.create_calculation(new_calc).await.expect("Should create calculation");

        let all = repo.list_calculations(None).await.expect("Should list all");
        assert_eq!(all.len(), 2);

        let for_2025 = repo.list_calculations(Some(2025)).await.expect("Should list for 2025");
        assert_eq!(for_2025.len(), 2);

        let for_2024 = repo.list_calculations(Some(2024)).await.expect("Should list for 2024");
        assert_eq!(for_2024.len(), 0);
    }
}
