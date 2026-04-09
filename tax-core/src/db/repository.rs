use async_trait::async_trait;
use thiserror::Error;

use crate::models::{
    FilingStatus, StandardDeduction, TaxBracket, TaxEstimate, TaxEstimateInput, TaxYearConfig,
};

#[derive(Debug, Error)]
pub enum RepositoryError {
    #[error("Record not found")]
    NotFound,

    #[error("Database error")]
    Database(#[source] anyhow::Error),

    #[error("Connection error")]
    Connection(#[source] anyhow::Error),

    /// Raised by the registry when no factory is registered for the
    /// requested backend name, or when required configuration is missing.
    #[error("Configuration error: {0}")]
    Configuration(String),

    /// A value was retrieved from the database but could not be parsed into
    /// the expected domain type (e.g. an unrecognised filing status code).
    #[error("Invalid data: {0}")]
    InvalidData(String),
}

#[async_trait]
pub trait TaxRepository: Send + Sync {
    // Tax year config
    async fn get_tax_year_config(
        &self,
        year: i32,
    ) -> Result<TaxYearConfig, RepositoryError>;
    async fn list_tax_years(&self) -> Result<Vec<i32>, RepositoryError>;

    // Filing status
    async fn get_filing_status(
        &self,
        id: i32,
    ) -> Result<FilingStatus, RepositoryError>;
    async fn get_filing_status_by_code(
        &self,
        code: &str,
    ) -> Result<FilingStatus, RepositoryError>;
    async fn list_filing_statuses(&self) -> Result<Vec<FilingStatus>, RepositoryError>;

    // Standard deductions
    async fn get_standard_deduction(
        &self,
        tax_year: i32,
        filing_status_id: i32,
    ) -> Result<StandardDeduction, RepositoryError>;

    /// Fetch every filing status together with its standard deduction and tax
    /// brackets for `year` via a single three-way JOIN, ordered by filing
    /// status id then bracket min income.
    async fn get_filing_status_data(
        &self,
        year: i32,
    ) -> Result<Vec<(FilingStatus, StandardDeduction, Vec<TaxBracket>)>, RepositoryError>;

    // Tax brackets
    async fn get_tax_brackets(
        &self,
        tax_year: i32,
        filing_status_id: i32,
    ) -> Result<Vec<TaxBracket>, RepositoryError>;

    async fn insert_tax_bracket(
        &self,
        bracket: &TaxBracket,
    ) -> Result<(), RepositoryError>;

    async fn delete_tax_brackets(
        &self,
        tax_year: i32,
        filing_status_id: i32,
    ) -> Result<(), RepositoryError>;

    // Tax estimates
    async fn create_estimate(
        &self,
        estimate: TaxEstimateInput,
    ) -> Result<TaxEstimate, RepositoryError>;

    async fn get_estimate(
        &self,
        id: i64,
    ) -> Result<TaxEstimate, RepositoryError>;

    async fn update_estimate(
        &self,
        estimate: &TaxEstimate,
    ) -> Result<(), RepositoryError>;

    async fn delete_estimate(
        &self,
        id: i64,
    ) -> Result<(), RepositoryError>;

    async fn list_estimates(
        &self,
        tax_year: Option<i32>,
    ) -> Result<Vec<TaxEstimate>, RepositoryError>;
}
