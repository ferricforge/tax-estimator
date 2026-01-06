use async_trait::async_trait;
use thiserror::Error;

use crate::models::{
    EstimatedTaxCalculation, FilingStatus, NewEstimatedTaxCalculation,
    StandardDeduction, TaxBracket, TaxYearConfig,
};

#[derive(Debug, Error)]
pub enum RepositoryError {
    #[error("Record not found")]
    NotFound,

    #[error("Database error: {0}")]
    Database(String),

    #[error("Connection error: {0}")]
    Connection(String),
}

#[async_trait]
pub trait TaxRepository: Send + Sync {
    // Tax year config
    async fn get_tax_year_config(&self, year: i32) -> Result<TaxYearConfig, RepositoryError>;
    async fn list_tax_years(&self) -> Result<Vec<i32>, RepositoryError>;

    // Filing status
    async fn get_filing_status(&self, id: i32) -> Result<FilingStatus, RepositoryError>;
    async fn list_filing_statuses(&self) -> Result<Vec<FilingStatus>, RepositoryError>;

    // Standard deductions
    async fn get_standard_deduction(
        &self,
        tax_year: i32,
        filing_status_id: i32,
    ) -> Result<StandardDeduction, RepositoryError>;

    // Tax brackets
    async fn get_tax_brackets(
        &self,
        tax_year: i32,
        filing_status_id: i32,
    ) -> Result<Vec<TaxBracket>, RepositoryError>;

    // Estimated tax calculations
    async fn create_calculation(
        &self,
        calc: NewEstimatedTaxCalculation,
    ) -> Result<EstimatedTaxCalculation, RepositoryError>;

    async fn get_calculation(&self, id: i64) -> Result<EstimatedTaxCalculation, RepositoryError>;

    async fn update_calculation(
        &self,
        calc: &EstimatedTaxCalculation,
    ) -> Result<(), RepositoryError>;

    async fn delete_calculation(&self, id: i64) -> Result<(), RepositoryError>;

    async fn list_calculations(
        &self,
        tax_year: Option<i32>,
    ) -> Result<Vec<EstimatedTaxCalculation>, RepositoryError>;
}