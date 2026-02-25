use std::io::Read;

use rust_decimal::Decimal;
use serde::Deserialize;
use tax_core::{RepositoryError, TaxBracket, TaxRepository};
use thiserror::Error;

/// Errors that can occur when loading tax bracket data.
#[derive(Debug, Error)]
pub enum TaxBracketLoaderError {
    #[error("CSV parse error: {0}")]
    CsvParse(String),

    #[error("Invalid schedule: {0}")]
    InvalidSchedule(String),

    #[error("Filing status '{0}' not found in database (have you run the seeds?)")]
    FilingStatusNotFound(String),

    #[error("Tax year {0} not found in database (have you run the seeds?)")]
    TaxYearNotFound(i32),

    #[error("Repository error: {0}")]
    Repository(#[from] RepositoryError),
}

impl From<csv::Error> for TaxBracketLoaderError {
    fn from(err: csv::Error) -> Self {
        TaxBracketLoaderError::CsvParse(err.to_string())
    }
}

/// Maps IRS schedule codes to filing status codes.
///
/// - Schedule X → Single (S)
/// - Schedule Y-1 → Married Filing Jointly (MFJ) and Qualifying Surviving Spouse (QSS)
/// - Schedule Y-2 → Married Filing Separately (MFS)
/// - Schedule Z → Head of Household (HOH)
fn schedule_to_filing_status_codes(
    schedule: &str
) -> Result<Vec<&'static str>, TaxBracketLoaderError> {
    match schedule {
        "X" => Ok(vec!["S"]),
        "Y-1" => Ok(vec!["MFJ", "QSS"]),
        "Y-2" => Ok(vec!["MFS"]),
        "Z" => Ok(vec!["HOH"]),
        _ => Err(TaxBracketLoaderError::InvalidSchedule(schedule.to_string())),
    }
}

/// A single record from the tax brackets CSV file.
///
/// The CSV format uses IRS schedule designations:
/// - `tax_year`: The tax year (e.g., 2025)
/// - `schedule`: The IRS schedule code (X, Y-1, Y-2, Z)
/// - `min_income`: The minimum income for this bracket
/// - `max_income`: The maximum income for this bracket (empty for unlimited)
/// - `base_tax`: The base tax amount for this bracket
/// - `rate`: The marginal tax rate as a decimal (e.g., 0.10 for 10%)
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct TaxBracketRecord {
    pub tax_year: i32,
    pub schedule: String,
    pub min_income: Decimal,
    #[serde(deserialize_with = "deserialize_optional_decimal")]
    pub max_income: Option<Decimal>,
    pub base_tax: Decimal,
    pub rate: Decimal,
}

fn deserialize_optional_decimal<'de, D>(deserializer: D) -> Result<Option<Decimal>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s: Option<String> = Option::deserialize(deserializer)?;
    match s {
        Some(s) if s.trim().is_empty() => Ok(None),
        Some(s) => s
            .trim()
            .parse::<Decimal>()
            .map(Some)
            .map_err(serde::de::Error::custom),
        None => Ok(None),
    }
}

/// Loader for tax bracket data from CSV files.
///
/// This loader reads CSV data and inserts it into the database via the
/// `TaxRepository` trait, allowing it to work with any database backend.
///
/// The CSV uses IRS schedule codes (X, Y-1, Y-2, Z) which are automatically
/// mapped to the appropriate filing status codes.
pub struct TaxBracketLoader;

impl TaxBracketLoader {
    /// Parse tax bracket records from a CSV reader.
    ///
    /// Returns a vector of parsed records. The reader can be any type that
    /// implements `Read`, such as a file or a string slice.
    pub fn parse<R: Read>(reader: R) -> Result<Vec<TaxBracketRecord>, TaxBracketLoaderError> {
        let mut csv_reader = csv::Reader::from_reader(reader);
        let mut records = Vec::new();

        for result in csv_reader.deserialize() {
            let record: TaxBracketRecord = result?;
            records.push(record);
        }

        Ok(records)
    }

    /// Load tax bracket records into the database.
    ///
    /// For each unique (tax_year, schedule) combination in the records,
    /// this method will:
    /// 1. Map the schedule to one or more filing status codes
    /// 2. Look up the filing status ID from each code
    /// 3. Delete any existing brackets for that year/status combination
    /// 4. Insert all new brackets for that combination
    ///
    /// This ensures that loading is idempotent - running the same load
    /// multiple times will produce the same result.
    ///
    /// Note: Schedule Y-1 maps to both MFJ and QSS, so those brackets
    /// will be duplicated for both filing statuses.
    pub async fn load<R: TaxRepository>(
        repo: &R,
        records: &[TaxBracketRecord],
    ) -> Result<usize, TaxBracketLoaderError> {
        let mut inserted = 0;

        // Group records by (tax_year, schedule) to delete and re-insert atomically
        let mut groups: std::collections::HashMap<(i32, String), Vec<&TaxBracketRecord>> =
            std::collections::HashMap::new();

        for record in records {
            groups
                .entry((record.tax_year, record.schedule.clone()))
                .or_default()
                .push(record);
        }

        for ((tax_year, schedule), group_records) in groups {
            // Map schedule to filing status codes
            let filing_status_codes = schedule_to_filing_status_codes(&schedule)?;

            for status_code in filing_status_codes {
                // Look up the filing status ID
                let filing_status =
                    repo.get_filing_status_by_code(status_code)
                        .await
                        .map_err(|e| match e {
                            RepositoryError::NotFound => {
                                TaxBracketLoaderError::FilingStatusNotFound(status_code.to_string())
                            }
                            other => TaxBracketLoaderError::Repository(other),
                        })?;

                // Delete existing brackets for this year/status
                repo.delete_tax_brackets(tax_year, filing_status.id).await?;

                // Insert new brackets
                for record in &group_records {
                    let bracket = TaxBracket {
                        tax_year: record.tax_year,
                        filing_status_id: filing_status.id,
                        min_income: record.min_income,
                        max_income: record.max_income,
                        tax_rate: record.rate,
                        base_tax: record.base_tax,
                    };

                    repo.insert_tax_bracket(&bracket).await.map_err(|e| {
                        if let RepositoryError::Database(ref inner) = e {
                            if inner.to_string().contains("FOREIGN KEY constraint failed") {
                                return TaxBracketLoaderError::TaxYearNotFound(record.tax_year);
                            }
                        }
                        TaxBracketLoaderError::Repository(e)
                    })?;
                    inserted += 1;
                }
            }
        }

        Ok(inserted)
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use rust_decimal_macros::dec;

    use super::*;

    const TEST_CSV: &str = r#"tax_year,schedule,min_income,max_income,base_tax,rate
2025,X,0,11925,0,0.10
2025,X,11925,48475,1192.50,0.12
2025,X,48475,103350,5578.50,0.22
2025,X,103350,197300,17651.00,0.24
2025,X,197300,250525,40199.00,0.32
2025,X,250525,626350,57231.00,0.35
2025,X,626350,,188769.75,0.37
2025,Y-1,0,23850,0,0.10
2025,Y-1,23850,96950,2385.00,0.12
2025,Y-1,96950,206700,11157.00,0.22
2025,Y-1,206700,394600,35302.00,0.24
2025,Y-1,394600,501050,80398.00,0.32
2025,Y-1,501050,751600,114462.00,0.35
2025,Y-1,751600,,202154.50,0.37
2025,Y-2,0,11925,0,0.10
2025,Y-2,11925,48475,1192.50,0.12
2025,Y-2,48475,103350,5578.50,0.22
2025,Y-2,103350,197300,17651.00,0.24
2025,Y-2,197300,250525,40199.00,0.32
2025,Y-2,250525,375800,57231.00,0.35
2025,Y-2,375800,,101077.25,0.37
2025,Z,0,17000,0,0.10
2025,Z,17000,64850,1700.00,0.12
2025,Z,64850,103350,7442.00,0.22
2025,Z,103350,197300,15912.00,0.24
2025,Z,197300,250500,38460.00,0.32
2025,Z,250500,626350,55484.00,0.35
2025,Z,626350,,187031.50,0.37
"#;

    #[test]
    fn test_parse_csv_single_bracket() {
        let csv = "tax_year,schedule,min_income,max_income,base_tax,rate\n2025,X,0,11925,0,0.10";

        let records = TaxBracketLoader::parse(csv.as_bytes()).expect("Failed to parse CSV");

        assert_eq!(records.len(), 1);
        assert_eq!(
            records[0],
            TaxBracketRecord {
                tax_year: 2025,
                schedule: "X".to_string(),
                min_income: dec!(0),
                max_income: Some(dec!(11925)),
                base_tax: dec!(0),
                rate: dec!(0.10),
            }
        );
    }

    #[test]
    fn test_parse_csv_unlimited_max_income() {
        let csv =
            "tax_year,schedule,min_income,max_income,base_tax,rate\n2025,X,626350,,188769.75,0.37";

        let records = TaxBracketLoader::parse(csv.as_bytes()).expect("Failed to parse CSV");

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].max_income, None);
        assert_eq!(records[0].min_income, dec!(626350));
        assert_eq!(records[0].base_tax, dec!(188769.75));
        assert_eq!(records[0].rate, dec!(0.37));
    }

    #[test]
    fn test_parse_csv_all_schedules() {
        let records = TaxBracketLoader::parse(TEST_CSV.as_bytes()).expect("Failed to parse CSV");

        assert_eq!(records.len(), 28);

        // Check we have all schedules
        let schedules: std::collections::HashSet<_> =
            records.iter().map(|r| r.schedule.as_str()).collect();
        assert!(schedules.contains("X"));
        assert!(schedules.contains("Y-1"));
        assert!(schedules.contains("Y-2"));
        assert!(schedules.contains("Z"));

        // Verify 7 brackets per schedule
        for schedule in ["X", "Y-1", "Y-2", "Z"] {
            let count = records.iter().filter(|r| r.schedule == schedule).count();
            assert_eq!(count, 7, "Expected 7 brackets for schedule {}", schedule);
        }
    }

    #[test]
    fn test_parse_schedule_x_single() {
        let records = TaxBracketLoader::parse(TEST_CSV.as_bytes()).expect("Failed to parse CSV");
        let single_brackets: Vec<_> = records.iter().filter(|r| r.schedule == "X").collect();

        assert_eq!(single_brackets.len(), 7);

        // Verify first bracket
        assert_eq!(single_brackets[0].min_income, dec!(0));
        assert_eq!(single_brackets[0].max_income, Some(dec!(11925)));
        assert_eq!(single_brackets[0].base_tax, dec!(0));
        assert_eq!(single_brackets[0].rate, dec!(0.10));

        // Verify second bracket
        assert_eq!(single_brackets[1].min_income, dec!(11925));
        assert_eq!(single_brackets[1].max_income, Some(dec!(48475)));
        assert_eq!(single_brackets[1].base_tax, dec!(1192.50));
        assert_eq!(single_brackets[1].rate, dec!(0.12));

        // Verify last bracket (unlimited)
        assert_eq!(single_brackets[6].min_income, dec!(626350));
        assert_eq!(single_brackets[6].max_income, None);
        assert_eq!(single_brackets[6].base_tax, dec!(188769.75));
        assert_eq!(single_brackets[6].rate, dec!(0.37));
    }

    #[test]
    fn test_parse_schedule_y1_mfj_qss() {
        let records = TaxBracketLoader::parse(TEST_CSV.as_bytes()).expect("Failed to parse CSV");
        let mfj_brackets: Vec<_> = records.iter().filter(|r| r.schedule == "Y-1").collect();

        assert_eq!(mfj_brackets.len(), 7);

        // Verify first bracket
        assert_eq!(mfj_brackets[0].min_income, dec!(0));
        assert_eq!(mfj_brackets[0].max_income, Some(dec!(23850)));
        assert_eq!(mfj_brackets[0].base_tax, dec!(0));
        assert_eq!(mfj_brackets[0].rate, dec!(0.10));

        // Verify last bracket
        assert_eq!(mfj_brackets[6].min_income, dec!(751600));
        assert_eq!(mfj_brackets[6].max_income, None);
        assert_eq!(mfj_brackets[6].base_tax, dec!(202154.50));
        assert_eq!(mfj_brackets[6].rate, dec!(0.37));
    }

    #[test]
    fn test_parse_schedule_y2_mfs() {
        let records = TaxBracketLoader::parse(TEST_CSV.as_bytes()).expect("Failed to parse CSV");
        let mfs_brackets: Vec<_> = records.iter().filter(|r| r.schedule == "Y-2").collect();

        assert_eq!(mfs_brackets.len(), 7);

        // MFS differs from Single in the 35% bracket max
        let bracket_35 = mfs_brackets.iter().find(|b| b.rate == dec!(0.35)).unwrap();
        assert_eq!(bracket_35.max_income, Some(dec!(375800)));

        // Last bracket
        assert_eq!(mfs_brackets[6].min_income, dec!(375800));
        assert_eq!(mfs_brackets[6].base_tax, dec!(101077.25));
    }

    #[test]
    fn test_parse_schedule_z_hoh() {
        let records = TaxBracketLoader::parse(TEST_CSV.as_bytes()).expect("Failed to parse CSV");
        let hoh_brackets: Vec<_> = records.iter().filter(|r| r.schedule == "Z").collect();

        assert_eq!(hoh_brackets.len(), 7);

        // HOH has different first bracket
        assert_eq!(hoh_brackets[0].min_income, dec!(0));
        assert_eq!(hoh_brackets[0].max_income, Some(dec!(17000)));
        assert_eq!(hoh_brackets[0].base_tax, dec!(0));

        // Second bracket
        assert_eq!(hoh_brackets[1].min_income, dec!(17000));
        assert_eq!(hoh_brackets[1].max_income, Some(dec!(64850)));
        assert_eq!(hoh_brackets[1].base_tax, dec!(1700.00));

        // Last bracket
        assert_eq!(hoh_brackets[6].min_income, dec!(626350));
        assert_eq!(hoh_brackets[6].base_tax, dec!(187031.50));
    }

    #[test]
    fn test_parse_invalid_csv_missing_column() {
        let csv = "tax_year,schedule,min_income\n2025,X,0";

        let result = TaxBracketLoader::parse(csv.as_bytes());

        let err = result.expect_err("Should fail for missing column");
        let TaxBracketLoaderError::CsvParse(msg) = err else {
            panic!("Expected CsvParse error, got: {:?}", err);
        };
        assert!(
            msg.contains("missing field"),
            "Expected 'missing field' in error, got: {}",
            msg
        );
    }

    #[test]
    fn test_parse_invalid_csv_bad_decimal() {
        let csv = "tax_year,schedule,min_income,max_income,base_tax,rate\n2025,X,abc,11925,0,0.10";

        let result = TaxBracketLoader::parse(csv.as_bytes());

        let err = result.expect_err("Should fail for invalid decimal");
        let TaxBracketLoaderError::CsvParse(msg) = err else {
            panic!("Expected CsvParse error, got: {:?}", err);
        };
        assert!(
            msg.contains("invalid"),
            "Expected 'invalid' in error, got: {}",
            msg
        );
    }

    #[test]
    fn test_parse_empty_csv() {
        let csv = "tax_year,schedule,min_income,max_income,base_tax,rate\n";

        let records = TaxBracketLoader::parse(csv.as_bytes()).expect("Failed to parse CSV");

        assert!(records.is_empty());
    }

    #[test]
    fn test_schedule_to_filing_status_codes_x() {
        let codes = schedule_to_filing_status_codes("X").expect("Should map X");

        assert_eq!(codes, vec!["S"]);
    }

    #[test]
    fn test_schedule_to_filing_status_codes_y1() {
        let codes = schedule_to_filing_status_codes("Y-1").expect("Should map Y-1");

        assert_eq!(codes, vec!["MFJ", "QSS"]);
    }

    #[test]
    fn test_schedule_to_filing_status_codes_y2() {
        let codes = schedule_to_filing_status_codes("Y-2").expect("Should map Y-2");

        assert_eq!(codes, vec!["MFS"]);
    }

    #[test]
    fn test_schedule_to_filing_status_codes_z() {
        let codes = schedule_to_filing_status_codes("Z").expect("Should map Z");

        assert_eq!(codes, vec!["HOH"]);
    }

    #[test]
    fn test_schedule_to_filing_status_codes_invalid() {
        let result = schedule_to_filing_status_codes("INVALID");

        match result {
            Err(TaxBracketLoaderError::InvalidSchedule(ref schedule)) => {
                assert_eq!(schedule, "INVALID");
            }
            other => panic!("expected InvalidSchedule, got {other:?}"),
        }
    }
}
