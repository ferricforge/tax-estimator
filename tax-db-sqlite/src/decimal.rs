use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use sqlx::{Row, TypeInfo, ValueRef};
use tax_core::RepositoryError;

/// Get a decimal value from a row, handling both INTEGER and REAL SQLite types.
pub fn get_decimal(row: &sqlx::sqlite::SqliteRow, column: &str) -> Result<Decimal, RepositoryError> {
    let value_ref = row
        .try_get_raw(column)
        .map_err(|e| RepositoryError::Database(format!("Column '{}' not found: {}", column, e)))?;

    let type_info = value_ref.type_info();
    let type_name = type_info.name();

    match type_name {
        "INTEGER" => {
            let val: i64 = row.try_get(column).map_err(|e| {
                RepositoryError::Database(format!(
                    "Failed to get INTEGER from '{}': {}",
                    column, e
                ))
            })?;
            Ok(Decimal::from(val))
        }
        "REAL" => {
            let val: f64 = row.try_get(column).map_err(|e| {
                RepositoryError::Database(format!("Failed to get REAL from '{}': {}", column, e))
            })?;
            Decimal::try_from(val).map_err(|e| {
                RepositoryError::Database(format!("Failed to convert {} to Decimal: {}", val, e))
            })
        }
        "NULL" => Ok(Decimal::ZERO),
        _ => Err(RepositoryError::Database(format!(
            "Unexpected type '{}' for column '{}'",
            type_name, column
        ))),
    }
}

/// Get an optional decimal value from a row, returning None for NULL values.
pub fn get_optional_decimal(
    row: &sqlx::sqlite::SqliteRow,
    column: &str,
) -> Result<Option<Decimal>, RepositoryError> {
    let value_ref = row
        .try_get_raw(column)
        .map_err(|e| RepositoryError::Database(format!("Column '{}' not found: {}", column, e)))?;

    if value_ref.is_null() {
        return Ok(None);
    }

    get_decimal(row, column).map(Some)
}

/// Convert a Decimal to f64 for SQLite storage.
pub fn decimal_to_f64(d: Decimal) -> f64 {
    d.to_f64().unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use rust_decimal_macros::dec;
    use sqlx::sqlite::SqlitePoolOptions;

    use super::*;

    async fn create_test_table(pool: &sqlx::sqlite::SqlitePool) {
        sqlx::query(
            "CREATE TABLE test_decimals (
                id INTEGER PRIMARY KEY,
                int_value INTEGER,
                real_value REAL,
                null_value REAL,
                text_value TEXT
            )",
        )
        .execute(pool)
        .await
        .expect("Failed to create test table");
    }

    async fn setup_test_db() -> sqlx::sqlite::SqlitePool {
        let pool = SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .expect("Failed to create in-memory database");
        create_test_table(&pool).await;
        pool
    }

    // get_decimal tests

    #[tokio::test]
    async fn test_get_decimal_from_integer() {
        let pool = setup_test_db().await;
        sqlx::query("INSERT INTO test_decimals (id, int_value) VALUES (1, 12345)")
            .execute(&pool)
            .await
            .expect("Failed to insert test data");

        let row = sqlx::query("SELECT int_value FROM test_decimals WHERE id = 1")
            .fetch_one(&pool)
            .await
            .expect("Failed to fetch row");

        let result = get_decimal(&row, "int_value");

        assert_eq!(result, Ok(dec!(12345)));
    }

    #[tokio::test]
    async fn test_get_decimal_from_negative_integer() {
        let pool = setup_test_db().await;
        sqlx::query("INSERT INTO test_decimals (id, int_value) VALUES (1, -99999)")
            .execute(&pool)
            .await
            .expect("Failed to insert test data");

        let row = sqlx::query("SELECT int_value FROM test_decimals WHERE id = 1")
            .fetch_one(&pool)
            .await
            .expect("Failed to fetch row");

        let result = get_decimal(&row, "int_value");

        assert_eq!(result, Ok(dec!(-99999)));
    }

    #[tokio::test]
    async fn test_get_decimal_from_real() {
        let pool = setup_test_db().await;
        sqlx::query("INSERT INTO test_decimals (id, real_value) VALUES (1, 123.45)")
            .execute(&pool)
            .await
            .expect("Failed to insert test data");

        let row = sqlx::query("SELECT real_value FROM test_decimals WHERE id = 1")
            .fetch_one(&pool)
            .await
            .expect("Failed to fetch row");

        let result = get_decimal(&row, "real_value");

        assert_eq!(result, Ok(dec!(123.45)));
    }

    #[tokio::test]
    async fn test_get_decimal_from_negative_real() {
        let pool = setup_test_db().await;
        sqlx::query("INSERT INTO test_decimals (id, real_value) VALUES (1, -456.78)")
            .execute(&pool)
            .await
            .expect("Failed to insert test data");

        let row = sqlx::query("SELECT real_value FROM test_decimals WHERE id = 1")
            .fetch_one(&pool)
            .await
            .expect("Failed to fetch row");

        let result = get_decimal(&row, "real_value");

        assert_eq!(result, Ok(dec!(-456.78)));
    }

    #[tokio::test]
    async fn test_get_decimal_from_null_returns_zero() {
        let pool = setup_test_db().await;
        sqlx::query("INSERT INTO test_decimals (id, null_value) VALUES (1, NULL)")
            .execute(&pool)
            .await
            .expect("Failed to insert test data");

        let row = sqlx::query("SELECT null_value FROM test_decimals WHERE id = 1")
            .fetch_one(&pool)
            .await
            .expect("Failed to fetch row");

        let result = get_decimal(&row, "null_value");

        assert_eq!(result, Ok(Decimal::ZERO));
    }

    #[tokio::test]
    async fn test_get_decimal_column_not_found() {
        let pool = setup_test_db().await;
        sqlx::query("INSERT INTO test_decimals (id) VALUES (1)")
            .execute(&pool)
            .await
            .expect("Failed to insert test data");

        let row = sqlx::query("SELECT id FROM test_decimals WHERE id = 1")
            .fetch_one(&pool)
            .await
            .expect("Failed to fetch row");

        let result = get_decimal(&row, "nonexistent_column");

        assert!(result.is_err());
        assert!(matches!(result, Err(RepositoryError::Database(msg)) if msg.starts_with("Column 'nonexistent_column' not found:")));
    }

    #[tokio::test]
    async fn test_get_decimal_unexpected_type() {
        let pool = setup_test_db().await;
        sqlx::query("INSERT INTO test_decimals (id, text_value) VALUES (1, 'not a number')")
            .execute(&pool)
            .await
            .expect("Failed to insert test data");

        let row = sqlx::query("SELECT text_value FROM test_decimals WHERE id = 1")
            .fetch_one(&pool)
            .await
            .expect("Failed to fetch row");

        let result = get_decimal(&row, "text_value");

        assert_eq!(
            result,
            Err(RepositoryError::Database(
                "Unexpected type 'TEXT' for column 'text_value'".to_string()
            ))
        );
    }

    // get_optional_decimal tests

    #[tokio::test]
    async fn test_get_optional_decimal_from_integer() {
        let pool = setup_test_db().await;
        sqlx::query("INSERT INTO test_decimals (id, int_value) VALUES (1, 54321)")
            .execute(&pool)
            .await
            .expect("Failed to insert test data");

        let row = sqlx::query("SELECT int_value FROM test_decimals WHERE id = 1")
            .fetch_one(&pool)
            .await
            .expect("Failed to fetch row");

        let result = get_optional_decimal(&row, "int_value");

        assert_eq!(result, Ok(Some(dec!(54321))));
    }

    #[tokio::test]
    async fn test_get_optional_decimal_from_real() {
        let pool = setup_test_db().await;
        sqlx::query("INSERT INTO test_decimals (id, real_value) VALUES (1, 999.99)")
            .execute(&pool)
            .await
            .expect("Failed to insert test data");

        let row = sqlx::query("SELECT real_value FROM test_decimals WHERE id = 1")
            .fetch_one(&pool)
            .await
            .expect("Failed to fetch row");

        let result = get_optional_decimal(&row, "real_value");

        assert_eq!(result, Ok(Some(dec!(999.99))));
    }

    #[tokio::test]
    async fn test_get_optional_decimal_from_null_returns_none() {
        let pool = setup_test_db().await;
        sqlx::query("INSERT INTO test_decimals (id, null_value) VALUES (1, NULL)")
            .execute(&pool)
            .await
            .expect("Failed to insert test data");

        let row = sqlx::query("SELECT null_value FROM test_decimals WHERE id = 1")
            .fetch_one(&pool)
            .await
            .expect("Failed to fetch row");

        let result = get_optional_decimal(&row, "null_value");

        assert_eq!(result, Ok(None));
    }

    #[tokio::test]
    async fn test_get_optional_decimal_column_not_found() {
        let pool = setup_test_db().await;
        sqlx::query("INSERT INTO test_decimals (id) VALUES (1)")
            .execute(&pool)
            .await
            .expect("Failed to insert test data");

        let row = sqlx::query("SELECT id FROM test_decimals WHERE id = 1")
            .fetch_one(&pool)
            .await
            .expect("Failed to fetch row");

        let result = get_optional_decimal(&row, "nonexistent_column");

        assert!(result.is_err());
        assert!(matches!(result, Err(RepositoryError::Database(msg)) if msg.starts_with("Column 'nonexistent_column' not found:")));
    }

    #[tokio::test]
    async fn test_get_optional_decimal_unexpected_type() {
        let pool = setup_test_db().await;
        sqlx::query("INSERT INTO test_decimals (id, text_value) VALUES (1, 'text')")
            .execute(&pool)
            .await
            .expect("Failed to insert test data");

        let row = sqlx::query("SELECT text_value FROM test_decimals WHERE id = 1")
            .fetch_one(&pool)
            .await
            .expect("Failed to fetch row");

        let result = get_optional_decimal(&row, "text_value");

        assert_eq!(
            result,
            Err(RepositoryError::Database(
                "Unexpected type 'TEXT' for column 'text_value'".to_string()
            ))
        );
    }

    // decimal_to_f64 tests

    #[test]
    fn test_decimal_to_f64_positive() {
        let decimal = dec!(123.456);

        let result = decimal_to_f64(decimal);

        assert_eq!(result, 123.456);
    }

    #[test]
    fn test_decimal_to_f64_negative() {
        let decimal = dec!(-789.012);

        let result = decimal_to_f64(decimal);

        assert_eq!(result, -789.012);
    }

    #[test]
    fn test_decimal_to_f64_zero() {
        let decimal = Decimal::ZERO;

        let result = decimal_to_f64(decimal);

        assert_eq!(result, 0.0);
    }

    #[test]
    fn test_decimal_to_f64_large_value() {
        let decimal = dec!(999999999.99);

        let result = decimal_to_f64(decimal);

        assert_eq!(result, 999999999.99);
    }

    #[test]
    fn test_decimal_to_f64_small_fraction() {
        let decimal = dec!(0.0001);

        let result = decimal_to_f64(decimal);

        assert_eq!(result, 0.0001);
    }
}
