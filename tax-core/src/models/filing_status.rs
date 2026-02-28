use serde::{Deserialize, Serialize};

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
