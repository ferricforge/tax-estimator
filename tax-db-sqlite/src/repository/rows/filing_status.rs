use tax_core::{FilingStatus, RepositoryError};

use super::parse_filing_status_code;

#[derive(sqlx::FromRow)]
pub(crate) struct FilingStatusRow {
    id: i32,
    status_code: String,
    status_name: String,
}

impl TryFrom<FilingStatusRow> for FilingStatus {
    type Error = RepositoryError;

    fn try_from(row: FilingStatusRow) -> Result<Self, Self::Error> {
        Ok(FilingStatus {
            id: row.id,
            status_code: parse_filing_status_code(&row.status_code)?,
            status_name: row.status_name,
        })
    }
}
