use sqlx::{Row, TypeInfo, ValueRef};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;

/// Encode: `Decimal` → `f64` for databases that store monetary values as
/// floating-point (e.g. SQLite `REAL`).
pub fn decimal_as_f64(d: &Decimal) -> f64 {
    d.to_f64().unwrap_or(0.0)
}

/// Decode: read a column as `f64` from a SQLite row and convert to `Decimal`.
pub fn decimal_from_sql(
    row: &sqlx::sqlite::SqliteRow,
    col: &str,
) -> Result<Decimal, sqlx::Error> {
    let raw = row.try_get_raw(col)?;
    match raw.type_info().name() {
        "INTEGER" => {
            let v: i64 = row.try_get(col)?;
            Ok(Decimal::from(v))
        }
        "REAL" => {
            let v: f64 = row.try_get(col)?;
            Decimal::try_from(v).map_err(|e| sqlx::Error::ColumnDecode {
                index: col.to_string(),
                source: Box::new(e),
            })
        }
        "NULL" => Ok(Decimal::ZERO),
        other => Err(sqlx::Error::ColumnDecode {
            index: col.to_string(),
            source: format!("unexpected SQLite type `{other}` for Decimal column").into(),
        }),
    }
}

pub fn optional_decimal_as_f64(d: &Option<Decimal>) -> Option<f64> {
    d.as_ref().map(|v| v.to_f64().unwrap_or(0.0))
}

pub fn optional_decimal_from_sql(
    row: &sqlx::sqlite::SqliteRow,
    col: &str,
) -> Result<Option<Decimal>, sqlx::Error> {
    let val: Option<f64> = sqlx::Row::try_get(row, col)?;
    val.map(|v| {
        Decimal::try_from(v).map_err(|e| sqlx::Error::ColumnDecode {
            index: col.to_string(),
            source: Box::new(e),
        })
    })
    .transpose()
}
