use serde::{Deserialize, Serialize};

const SINGLE: i32 = 1;
const MFJ: i32 = 2;
const MFS: i32 = 3;
const HOH: i32 = 4;
const QSS: i32 = 5;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FilingStatusCode {
    #[default]
    Single,
    MarriedFilingJointly,
    MarriedFilingSeparately,
    HeadOfHousehold,
    QualifyingSurvivingSpouse,
}

impl FilingStatusCode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Single => "S",
            Self::MarriedFilingJointly => "MFJ",
            Self::MarriedFilingSeparately => "MFS",
            Self::HeadOfHousehold => "HOH",
            Self::QualifyingSurvivingSpouse => "QSS",
        }
    }

    /// Returns the long display name for this filing status (e.g. "Married Filing Jointly").
    pub fn to_long_str(&self) -> &'static str {
        match self {
            Self::Single => "Single",
            Self::MarriedFilingJointly => "Married Filing Jointly",
            Self::MarriedFilingSeparately => "Married Filing Separately",
            Self::HeadOfHousehold => "Head of Household",
            Self::QualifyingSurvivingSpouse => "Qualifying Surviving Spouse",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "S" => Some(Self::Single),
            "MFJ" => Some(Self::MarriedFilingJointly),
            "MFS" => Some(Self::MarriedFilingSeparately),
            "HOH" => Some(Self::HeadOfHousehold),
            "QSS" => Some(Self::QualifyingSurvivingSpouse),
            _ => None,
        }
    }

    /// ---------------------------------------------------------------------------
    /// Filing-status code → seed ID mapping
    /// ---------------------------------------------------------------------------
    /// This mirrors the IDs established by 01_filing_status.sql.  If the seed
    /// data ever changes the mapping lives in exactly one place.
    pub fn filing_status_to_id(code: FilingStatusCode) -> i32 {
        match code {
            FilingStatusCode::Single => SINGLE,
            FilingStatusCode::MarriedFilingJointly => MFJ,
            FilingStatusCode::MarriedFilingSeparately => MFS,
            FilingStatusCode::HeadOfHousehold => HOH,
            FilingStatusCode::QualifyingSurvivingSpouse => QSS,
        }
    }
}

impl TryFrom<&str> for FilingStatusCode {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "S" | "Single" => Ok(Self::Single),
            "MFJ" | "Married Filing Jointly" => Ok(Self::MarriedFilingJointly),
            "MFS" | "Married Filing Separately" => Ok(Self::MarriedFilingSeparately),
            "HOH" | "Head of Household" => Ok(Self::HeadOfHousehold),
            "QSS" | "Qualifying Surviving Spouse" => Ok(Self::QualifyingSurvivingSpouse),
            _ => Err(anyhow::anyhow!("Unknown filing status: '{value}'")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FilingStatus {
    pub id: i32,
    pub status_code: FilingStatusCode,
    pub status_name: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_filing_status_to_id_single() {
        assert_eq!(
            FilingStatusCode::filing_status_to_id(FilingStatusCode::Single),
            SINGLE
        );
    }

    #[test]
    fn test_filing_status_to_id_married_joint() {
        assert_eq!(
            FilingStatusCode::filing_status_to_id(FilingStatusCode::MarriedFilingJointly),
            MFJ
        );
    }

    #[test]
    fn test_filing_status_to_id_married_separate() {
        assert_eq!(
            FilingStatusCode::filing_status_to_id(FilingStatusCode::MarriedFilingSeparately),
            MFS
        );
    }

    #[test]
    fn test_filing_status_to_id_head_of_household() {
        assert_eq!(
            FilingStatusCode::filing_status_to_id(FilingStatusCode::HeadOfHousehold),
            HOH
        );
    }

    #[test]
    fn test_filing_status_to_id_qualifying() {
        assert_eq!(
            FilingStatusCode::filing_status_to_id(FilingStatusCode::QualifyingSurvivingSpouse),
            QSS
        );
    }
}
