//! Input validation for worksheet forms.
//!
//! Validation lives in `tax-core` so every front end applies the same rules to
//! the same inputs. The GUI reads these messages, and any other crate can call
//! the same functions directly.
//!
//! - [`rules`] parses raw field text into values and checks format, sign,
//!   length, and bounds.
//! - [`report`] collects the resulting messages per field and tracks whether
//!   any of them block calculation.
//! - [`worksheet`] defines the contract each worksheet validator implements.
//! - One module per worksheet ([`se_worksheet`] and [`qbi_worksheet`]) defines
//!   its field keys, raw and parsed types, and validator.

pub mod qbi_worksheet;
pub mod report;
pub mod rules;
pub mod se_worksheet;
pub mod worksheet;

pub use qbi_worksheet::{
    QbiField, QbiRawInputs, QbiRawTrade, QbiTradeInput, QbiWorksheetInputs, QbiWorksheetValidator,
    TradeColumn,
};
pub use report::{Severity, ValidationIssue, ValidationReport};
pub use rules::{DecimalFieldRules, TextFieldRules, max_money, money};
pub use se_worksheet::{SeField, SeRawInputs, SeWorksheetInputs, SeWorksheetValidator};
pub use worksheet::{Validated, Worksheet};
