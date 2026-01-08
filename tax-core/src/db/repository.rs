use async_trait::async_trait;
use thiserror::Error;

use crate::models::{
    FilingStatus, NewTaxEstimate, StandardDeduction, TaxBracket, TaxEstimate, TaxYearConfig,
};

#[derive(Debug, Error, PartialEq, Eq)]
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

    // Tax estimates
    async fn create_estimate(&self, estimate: NewTaxEstimate)
        -> Result<TaxEstimate, RepositoryError>;

    async fn get_estimate(&self, id: i64) -> Result<TaxEstimate, RepositoryError>;

    async fn update_estimate(&self, estimate: &TaxEstimate) -> Result<(), RepositoryError>;

    async fn delete_estimate(&self, id: i64) -> Result<(), RepositoryError>;

    async fn list_estimates(&self, tax_year: Option<i32>)
        -> Result<Vec<TaxEstimate>, RepositoryError>;
}
