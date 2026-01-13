//! Common utility functions for tax calculations.
//!
//! This module provides shared functionality used across multiple worksheet
//! calculations, including rounding and other common operations.

use rust_decimal::Decimal;

/// Rounds a decimal value to exactly two decimal places using half-up rounding.
///
/// This follows standard financial rounding conventions where values at exactly
/// 0.005 are rounded up to 0.01 (away from zero).
///
/// # Arguments
///
/// * `value` - The decimal value to round
///
/// # Returns
///
/// The value rounded to two decimal places.
///
/// # Examples
///
/// ```
/// use rust_decimal_macros::dec;
/// use tax_core::calculations::common::round_half_up;
///
/// assert_eq!(round_half_up(dec!(123.454)), dec!(123.45));
/// assert_eq!(round_half_up(dec!(123.455)), dec!(123.46));
/// assert_eq!(round_half_up(dec!(123.456)), dec!(123.46));
/// assert_eq!(round_half_up(dec!(-123.455)), dec!(-123.46)); // Away from zero
/// ```
pub fn round_half_up(value: Decimal) -> Decimal {
    value.round_dp_with_strategy(2, rust_decimal::RoundingStrategy::MidpointAwayFromZero)
}

/// Returns the maximum of two decimal values.
///
/// # Arguments
///
/// * `a` - First decimal value
/// * `b` - Second decimal value
///
/// # Returns
///
/// The larger of the two values.
///
/// # Examples
///
/// ```
/// use rust_decimal_macros::dec;
/// use tax_core::calculations::common::max;
///
/// assert_eq!(max(dec!(100.00), dec!(200.00)), dec!(200.00));
/// assert_eq!(max(dec!(200.00), dec!(100.00)), dec!(200.00));
/// assert_eq!(max(dec!(-100.00), dec!(-200.00)), dec!(-100.00));
/// ```
pub fn max(
    a: Decimal,
    b: Decimal,
) -> Decimal {
    if a > b { a } else { b }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use rust_decimal_macros::dec;

    use super::*;

    // =========================================================================
    // round_half_up tests
    // =========================================================================

    #[test]
    fn round_half_up_rounds_down_below_midpoint() {
        let result = round_half_up(dec!(123.454));

        assert_eq!(result, dec!(123.45));
    }

    #[test]
    fn round_half_up_rounds_up_at_midpoint() {
        let result = round_half_up(dec!(123.455));

        assert_eq!(result, dec!(123.46));
    }

    #[test]
    fn round_half_up_rounds_up_above_midpoint() {
        let result = round_half_up(dec!(123.456));

        assert_eq!(result, dec!(123.46));
    }

    #[test]
    fn round_half_up_handles_negative_values() {
        let result = round_half_up(dec!(-123.455));

        assert_eq!(result, dec!(-123.46)); // Away from zero
    }

    #[test]
    fn round_half_up_preserves_already_rounded_values() {
        let result = round_half_up(dec!(123.45));

        assert_eq!(result, dec!(123.45));
    }

    #[test]
    fn round_half_up_handles_zero() {
        let result = round_half_up(dec!(0.00));

        assert_eq!(result, dec!(0.00));
    }

    #[test]
    fn round_half_up_handles_small_values() {
        let result = round_half_up(dec!(0.001));

        assert_eq!(result, dec!(0.00));
    }

    #[test]
    fn round_half_up_handles_large_values() {
        let result = round_half_up(dec!(999999.999));

        assert_eq!(result, dec!(1000000.00));
    }

    // =========================================================================
    // max tests
    // =========================================================================

    #[test]
    fn max_returns_larger_value() {
        let result = max(dec!(100.00), dec!(200.00));

        assert_eq!(result, dec!(200.00));
    }

    #[test]
    fn max_returns_first_when_larger() {
        let result = max(dec!(200.00), dec!(100.00));

        assert_eq!(result, dec!(200.00));
    }

    #[test]
    fn max_handles_equal_values() {
        let result = max(dec!(150.00), dec!(150.00));

        assert_eq!(result, dec!(150.00));
    }

    #[test]
    fn max_handles_negative_values() {
        let result = max(dec!(-100.00), dec!(-200.00));

        assert_eq!(result, dec!(-100.00));
    }

    #[test]
    fn max_handles_zero() {
        let result = max(dec!(0.00), dec!(100.00));

        assert_eq!(result, dec!(100.00));
    }

    #[test]
    fn max_handles_negative_and_positive() {
        let result = max(dec!(-50.00), dec!(50.00));

        assert_eq!(result, dec!(50.00));
    }
}
