use rust_decimal::Decimal;
use thiserror::Error;

/// Error returned when a string cannot be parsed as a [`Decimal`].
#[derive(Debug, Error)]
#[error("invalid decimal '{input}': {source}")]
pub struct ParseDecimalError {
    input: String,
    #[source]
    source: rust_decimal::Error,
}

/// Normalizes input for decimal parsing: trims whitespace and removes commas (thousands separator).
fn normalize_decimal_input(s: &str) -> String {
    s.trim().replace(',', "")
}

/// Parses a string into a [`Decimal`].
///
/// Handles comma as thousands separator (e.g. `"1,234.56"`).
/// Empty or whitespace-only input is treated as 0.
/// Returns an error and logs when the input is invalid (non-empty but not parseable).
pub fn parse_decimal(s: &str) -> Result<Decimal, ParseDecimalError> {
    let normalized = normalize_decimal_input(s);
    if normalized.is_empty() {
        return Ok(Decimal::ZERO);
    }
    normalized.parse().map_err(|e| {
        tracing::error!(input = %s, "invalid decimal: {}", e);
        ParseDecimalError {
            input: s.to_string(),
            source: e,
        }
    })
}

/// Parses a string into an optional [`Decimal`].
///
/// Handles comma as thousands separator. Returns `None` for empty or whitespace-only input,
/// or when parsing fails (logs a warning on parse failure).
pub fn parse_optional_decimal(s: &str) -> Option<Decimal> {
    let normalized = normalize_decimal_input(s);
    if normalized.is_empty() {
        None
    } else {
        normalized.parse().map_or_else(
            |e| {
                tracing::warn!(input = %s, "invalid optional decimal: {}", e);
                None
            },
            Some,
        )
    }
}

/// Formats an optional [`Decimal`] for display, using "—" when `None`.
pub fn opt_decimal_display(d: &Option<Decimal>) -> String {
    d.as_ref()
        .map(|v| v.to_string())
        .unwrap_or_else(|| "—".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn parse_decimal_accepts_comma_thousands_separator() {
        assert_eq!(parse_decimal("1,234.56").unwrap(), dec!(1234.56));
        assert_eq!(parse_decimal("1,234,567.89").unwrap(), dec!(1234567.89));
    }

    #[test]
    fn parse_decimal_trim_whitespace() {
        assert_eq!(parse_decimal("  123.45  ").unwrap(), dec!(123.45));
    }

    #[test]
    fn parse_decimal_empty_treated_as_zero() {
        assert_eq!(parse_decimal("").unwrap(), Decimal::ZERO);
        assert_eq!(parse_decimal("   ").unwrap(), Decimal::ZERO);
    }

    #[test]
    fn parse_decimal_invalid_returns_error() {
        assert!(parse_decimal("abc").is_err());
    }

    #[test]
    fn parse_optional_decimal_handles_comma_and_empty() {
        assert_eq!(parse_optional_decimal("1,234.56"), Some(dec!(1234.56)));
        assert_eq!(parse_optional_decimal(""), None);
        assert_eq!(parse_optional_decimal("   "), None);
    }
}
