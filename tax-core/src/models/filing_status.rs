use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FilingStatusCode {
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilingStatus {
    pub id: i32,
    pub status_code: FilingStatusCode,
    pub status_name: String,
}
