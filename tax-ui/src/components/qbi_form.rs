//! Form 8995 worksheet: the simplified QBI deduction computation.
//!
//! Line 11 comes from the estimate form. Lines 2 through 17 are computed from
//! the entered values and update on each change.

use gpui::{
    App, Context, Div, Entity, EventEmitter, InteractiveElement as _, IntoElement, ParentElement,
    Render, StatefulInteractiveElement as _, Styled, TextAlign, Window, div, px,
};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::{ActiveTheme, h_flex, v_flex};
use rust_decimal::{Decimal, RoundingStrategy};

use crate::components::{
    ErrorDialog, make_button, make_carryforward_display_row_with_help, make_decimal_input,
    make_display_row, make_display_row_with_help, make_header_row, make_help_slot,
    make_input_row_fixed_with_help, make_text_input, set_input_value,
};
use crate::instructions::{UiInstructionField, help_for_field};
use crate::utils::parse_optional_decimal;

/// Width of the line 1 row index (for example, "i" or "iii").
const INDEX_WIDTH: f32 = 36.0;
/// Width of the trade, business, or aggregation name column.
const TRADE_NAME_WIDTH: f32 = 230.0;
/// Width of the taxpayer identification number column.
const TRADE_TIN_WIDTH: f32 = 120.0;
/// Width of the qualified business income column.
const TRADE_QBI_WIDTH: f32 = 150.0;
/// Maximum height of the scrolling body before the dialog scrolls.
const BODY_MAX_HEIGHT: f32 = 600.0;
/// Rate applied to qualified business income and REIT/PTP amounts.
const QBI_RATE: Decimal = Decimal::from_parts(20, 0, 0, false, 2);
/// Row labels for the five trade, business, or aggregation rows (line 1).
const TRADE_ROW_INDEXES: [&str; 5] = ["i", "ii", "iii", "iv", "v"];

/// Events the QBI form sends to the window that owns it.
pub enum QbiFormEvent {
    /// The user chose to use line 15 as the estimate's QBI deduction.
    ApplyQbiDeduction(Decimal),
}

/// One row of the line 1 table.
struct TradeRow {
    name: Entity<InputState>,
    taxpayer_id: Entity<InputState>,
    qbi: Entity<InputState>,
}

/// Values the Form 8995 lines are computed from.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct QbiInputs {
    /// Line 2: sum of the line 1 column (c) values.
    trade_total: Decimal,
    /// Line 3: QBI net loss carryforward from the prior year.
    line_3: Decimal,
    /// Line 6: qualified REIT dividends and PTP income or loss.
    line_6: Decimal,
    /// Line 7: qualified REIT dividends and PTP loss carryforward.
    line_7: Decimal,
    /// Line 11: taxable income before the QBI deduction.
    taxable_income: Option<Decimal>,
    /// Line 12: net capital gain increased by qualified dividends.
    line_12: Decimal,
}

/// Computed Form 8995 lines. `None` means the line cannot be computed yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct QbiLines {
    line_2: Decimal,
    line_4: Decimal,
    line_5: Decimal,
    line_8: Decimal,
    line_9: Decimal,
    line_10: Decimal,
    line_11: Option<Decimal>,
    line_13: Option<Decimal>,
    line_14: Option<Decimal>,
    line_15: Decimal,
    line_16: Decimal,
    line_17: Decimal,
}

pub struct QbiForm {
    trade_rows: Vec<TradeRow>,
    /// Line 3: QBI net loss carryforward from the prior year.
    line_3_carryforward: Entity<InputState>,
    /// Line 6: qualified REIT dividends and PTP income or loss.
    line_6_reit_ptp: Entity<InputState>,
    /// Line 7: qualified REIT dividends and PTP loss carryforward.
    line_7_reit_ptp_carryforward: Entity<InputState>,
    /// Line 12: net capital gain increased by qualified dividends.
    line_12_capital_gain_dividends: Entity<InputState>,
    /// Line 11: taxable income before the QBI deduction, from the estimate.
    taxable_income: Option<Decimal>,
    lines: QbiLines,
    tax_year: Option<i32>,
    threshold: Option<Decimal>,
    over_threshold: bool,
}

impl EventEmitter<QbiFormEvent> for QbiForm {}

impl QbiForm {
    pub fn new(
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let trade_rows: Vec<TradeRow> = TRADE_ROW_INDEXES
            .iter()
            .map(|_| TradeRow {
                name: make_text_input("Trade or business name", window, cx),
                taxpayer_id: make_text_input("Taxpayer ID", window, cx),
                qbi: make_decimal_input("QBI or (loss)", 2, window, cx),
            })
            .collect();
        let line_3_carryforward = make_decimal_input("QBI carryforward", 2, window, cx);
        let line_6_reit_ptp = make_decimal_input("REIT and PTP income", 2, window, cx);
        let line_7_reit_ptp_carryforward =
            make_decimal_input("REIT and PTP carryforward", 2, window, cx);
        let line_12_capital_gain_dividends =
            make_decimal_input("Capital gain and dividends", 2, window, cx);

        let mut watched = vec![
            line_3_carryforward.clone(),
            line_6_reit_ptp.clone(),
            line_7_reit_ptp_carryforward.clone(),
            line_12_capital_gain_dividends.clone(),
        ];
        watched.extend(trade_rows.iter().map(|row| row.qbi.clone()));

        for input in &watched {
            cx.subscribe(input, |this, _input, event, cx| {
                if let InputEvent::Change = event {
                    this.recalculate(cx);
                }
            })
            .detach();
        }

        Self {
            trade_rows,
            line_3_carryforward,
            line_6_reit_ptp,
            line_7_reit_ptp_carryforward,
            line_12_capital_gain_dividends,
            taxable_income: None,
            lines: compute_lines(QbiInputs::default()),
            tax_year: None,
            threshold: None,
            over_threshold: false,
        }
    }

    /// Sets the tax year, filing status, and the estimate values that make up
    /// taxable income before the QBI deduction, then checks the threshold.
    pub fn set_context(
        &mut self,
        tax_year: Option<i32>,
        is_joint: bool,
        expected_agi: Option<Decimal>,
        expected_deduction: Option<Decimal>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.tax_year = tax_year;
        self.threshold = taxable_income_threshold(tax_year, is_joint);
        self.taxable_income = taxable_income_before_qbi(expected_agi, expected_deduction);
        self.recalculate(cx);
        self.check_threshold(window, cx);
    }

    /// Empties every value the user enters and recalculates the lines. The
    /// taxable income from the estimate is kept.
    fn clear(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        for row in &self.trade_rows {
            set_input_value(&row.name, "", window, cx);
            set_input_value(&row.taxpayer_id, "", window, cx);
            set_input_value(&row.qbi, "", window, cx);
        }
        for input in [
            &self.line_3_carryforward,
            &self.line_6_reit_ptp,
            &self.line_7_reit_ptp_carryforward,
            &self.line_12_capital_gain_dividends,
        ] {
            set_input_value(input, "", window, cx);
        }
        self.recalculate(cx);
    }

    /// Sends line 15 to the owning window so it can fill the estimate's QBI
    /// deduction field.
    fn apply_deduction(
        &self,
        cx: &mut Context<Self>,
    ) {
        cx.emit(QbiFormEvent::ApplyQbiDeduction(self.lines.line_15));
    }

    /// Recomputes every computed line from the current inputs.
    fn recalculate(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        let trade_values: Vec<Option<Decimal>> = self
            .trade_rows
            .iter()
            .map(|row| read_optional(&row.qbi, cx))
            .collect();

        let inputs = QbiInputs {
            trade_total: sum_trade_values(trade_values),
            line_3: read_decimal(&self.line_3_carryforward, cx),
            line_6: read_decimal(&self.line_6_reit_ptp, cx),
            line_7: read_decimal(&self.line_7_reit_ptp_carryforward, cx),
            taxable_income: self.taxable_income,
            line_12: read_decimal(&self.line_12_capital_gain_dividends, cx),
        };
        self.lines = compute_lines(inputs);
        cx.notify();
    }

    /// Shows an alert when taxable income is above the threshold, and sets the
    /// inline warning to match.
    fn check_threshold(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.over_threshold = exceeds_threshold(self.taxable_income, self.threshold);
        if self.over_threshold {
            let message = self.threshold_warning();
            ErrorDialog::show("Taxable income above threshold", &[message], window, cx);
        }
        cx.notify();
    }

    fn threshold_warning(&self) -> String {
        let threshold = self.threshold.unwrap_or_default();
        format!("Taxable income is above the ${threshold:.0} threshold. Use Form 8995-A instead.")
    }

    fn render_note(&self) -> Div {
        div().text_size(px(12.0)).child(
            "Note: Use this form only if taxable income before the QBI deduction is at or \
             below the threshold for your filing status, and you are not a patron of an \
             agricultural or horticultural cooperative.",
        )
    }

    fn render_threshold_warning(
        &self,
        cx: &App,
    ) -> Div {
        if !self.over_threshold {
            return div();
        }

        div()
            .p_2()
            .rounded_md()
            .border_1()
            .border_color(cx.theme().warning)
            .text_color(cx.theme().warning)
            .child(self.threshold_warning())
    }

    fn render_trade_table(&self) -> Div {
        let line_1_help = help_for_field(UiInstructionField::QbiLine1, self.tax_year);

        let header = h_flex()
            .gap_2()
            .p(px(2.))
            .child(div().w(px(INDEX_WIDTH)))
            .child(
                div()
                    .w(px(TRADE_NAME_WIDTH))
                    .text_align(TextAlign::Center)
                    .child("Trade, business, or aggregation name"),
            )
            .child(
                div()
                    .w(px(TRADE_TIN_WIDTH))
                    .text_align(TextAlign::Center)
                    .child("Taxpayer ID"),
            )
            .child(
                div()
                    .w(px(TRADE_QBI_WIDTH))
                    .text_align(TextAlign::Center)
                    .child("QBI or (loss)"),
            )
            .child(make_help_slot(line_1_help));

        v_flex().gap_1().child(header).children(
            self.trade_rows
                .iter()
                .zip(TRADE_ROW_INDEXES)
                .map(|(row, index)| make_trade_row(index, row)),
        )
    }

    fn render_lines(&self) -> Div {
        let year = self.tax_year;
        let lines = self.lines;

        v_flex()
            .gap_2()
            .child(make_display_row_with_help(
                "2. Total qualified business income or (loss):",
                Some(lines.line_2),
                help_for_field(UiInstructionField::QbiLine2, year),
            ))
            .child(make_input_row_fixed_with_help(
                &self.line_3_carryforward,
                "3. QBI net (loss) carryforward from prior year: $",
                help_for_field(UiInstructionField::QbiLine3, year),
            ))
            .child(make_display_row_with_help(
                "4. Total qualified business income (2 + 3):",
                Some(lines.line_4),
                help_for_field(UiInstructionField::QbiLine4, year),
            ))
            .child(make_display_row(
                "5. QBI component (4 × 20%):",
                Some(lines.line_5),
            ))
            .child(make_input_row_fixed_with_help(
                &self.line_6_reit_ptp,
                "6. Qualified REIT dividends and PTP income or (loss): $",
                help_for_field(UiInstructionField::QbiLine6, year),
            ))
            .child(make_input_row_fixed_with_help(
                &self.line_7_reit_ptp_carryforward,
                "7. Qualified REIT and PTP (loss) carryforward: $",
                help_for_field(UiInstructionField::QbiLine7, year),
            ))
            .child(make_display_row_with_help(
                "8. Total qualified REIT dividends and PTP income (6 + 7):",
                Some(lines.line_8),
                help_for_field(UiInstructionField::QbiLine8, year),
            ))
            .child(make_display_row(
                "9. REIT and PTP component (8 × 20%):",
                Some(lines.line_9),
            ))
            .child(make_display_row(
                "10. QBI deduction before the income limitation (5 + 9):",
                Some(lines.line_10),
            ))
            .child(make_display_row_with_help(
                "11. Taxable income before QBI deduction:",
                lines.line_11,
                help_for_field(UiInstructionField::QbiLine11, year),
            ))
            .child(make_input_row_fixed_with_help(
                &self.line_12_capital_gain_dividends,
                "12. Net capital gain plus qualified dividends: $",
                help_for_field(UiInstructionField::QbiLine12, year),
            ))
            .child(make_display_row(
                "13. Subtract line 12 from line 11:",
                lines.line_13,
            ))
            .child(make_display_row(
                "14. Income limitation (13 × 20%):",
                lines.line_14,
            ))
            .child(make_display_row_with_help(
                "15. QBI deduction (smaller of line 10 or line 14):",
                Some(lines.line_15),
                help_for_field(UiInstructionField::QbiLine15, year),
            ))
            .child(make_carryforward_display_row_with_help(
                "16. Total qualified business (loss) carryforward:",
                Some(lines.line_16),
                help_for_field(UiInstructionField::QbiLine16, year),
            ))
            .child(make_carryforward_display_row_with_help(
                "17. Total qualified REIT and PTP (loss) carryforward:",
                Some(lines.line_17),
                help_for_field(UiInstructionField::QbiLine17, year),
            ))
    }
}

impl Render for QbiForm {
    fn render(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let this = cx.entity().clone();

        div()
            .id("qbi-form-body")
            .overflow_y_scroll()
            .max_h(px(BODY_MAX_HEIGHT))
            .child(
                v_flex()
                    .gap_3()
                    .p_4()
                    .child(self.render_note())
                    .child(self.render_threshold_warning(cx))
                    .child(make_header_row("Line 1: Trades or Businesses:"))
                    .child(self.render_trade_table())
                    .child(self.render_lines())
                    .child(
                        h_flex()
                            .gap_2()
                            .justify_end()
                            .mt_4()
                            .child(make_button("qbi_clear", "Clear", true, {
                                let this = this.clone();
                                move |_ev, window, app_cx| {
                                    this.update(app_cx, |form, cx| {
                                        form.clear(window, cx);
                                    });
                                }
                            }))
                            .child(make_button(
                                "qbi_apply",
                                "Apply to Estimate",
                                true,
                                move |_ev, _window, app_cx| {
                                    this.update(app_cx, |form, cx| {
                                        form.apply_deduction(cx);
                                    });
                                },
                            )),
                    ),
            )
    }
}

fn make_trade_row(
    index: &'static str,
    row: &TradeRow,
) -> Div {
    h_flex()
        .items_center()
        .gap_2()
        .p(px(2.))
        .child(
            div()
                .w(px(INDEX_WIDTH))
                .text_align(TextAlign::Right)
                .child(index),
        )
        .child(Input::new(&row.name).w(px(TRADE_NAME_WIDTH)))
        .child(Input::new(&row.taxpayer_id).w(px(TRADE_TIN_WIDTH)))
        .child(Input::new(&row.qbi).w(px(TRADE_QBI_WIDTH)))
        .child(make_help_slot(None))
}

/// Reads an input as a decimal, or `None` when blank or unreadable.
fn read_optional(
    input: &Entity<InputState>,
    cx: &App,
) -> Option<Decimal> {
    parse_optional_decimal(input.read(cx).value().as_str())
}

/// Reads an input as a decimal. Blank or unreadable text counts as zero.
fn read_decimal(
    input: &Entity<InputState>,
    cx: &App,
) -> Decimal {
    read_optional(input, cx).unwrap_or(Decimal::ZERO)
}

/// Sums the line 1 column (c) values. A blank entry counts as zero.
fn sum_trade_values(values: impl IntoIterator<Item = Option<Decimal>>) -> Decimal {
    values
        .into_iter()
        .map(|value| value.unwrap_or(Decimal::ZERO))
        .sum()
}

/// Rounds an amount to cents, with halves rounded away from zero.
fn to_cents(amount: Decimal) -> Decimal {
    amount.round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero)
}

/// Taxable income before the QBI deduction: Form 1040 line 11a minus line 12e.
/// Line 13b is not included because the application does not collect it yet.
fn taxable_income_before_qbi(
    expected_agi: Option<Decimal>,
    expected_deduction: Option<Decimal>,
) -> Option<Decimal> {
    Some(expected_agi? - expected_deduction?)
}

/// Computes the Form 8995 lines from the entered values.
fn compute_lines(inputs: QbiInputs) -> QbiLines {
    let line_4 = (inputs.trade_total + inputs.line_3).max(Decimal::ZERO);
    let line_5 = to_cents(line_4 * QBI_RATE);
    let line_8 = (inputs.line_6 + inputs.line_7).max(Decimal::ZERO);
    let line_9 = to_cents(line_8 * QBI_RATE);
    let line_10 = line_5 + line_9;
    let line_13 = inputs
        .taxable_income
        .map(|income| (income - inputs.line_12).max(Decimal::ZERO));
    let line_14 = line_13.map(|amount| to_cents(amount * QBI_RATE));
    let line_15 = line_10.min(line_14.unwrap_or(Decimal::ZERO));
    let line_16 = (inputs.trade_total + inputs.line_3).min(Decimal::ZERO);
    let line_17 = (inputs.line_6 + inputs.line_7).min(Decimal::ZERO);

    QbiLines {
        line_2: inputs.trade_total,
        line_4,
        line_5,
        line_8,
        line_9,
        line_10,
        line_11: inputs.taxable_income,
        line_13,
        line_14,
        line_15,
        line_16,
        line_17,
    }
}

/// Returns `true` when taxable income is strictly above the threshold. A
/// missing value on either side never counts as above.
fn exceeds_threshold(
    income: Option<Decimal>,
    threshold: Option<Decimal>,
) -> bool {
    match (income, threshold) {
        (Some(income), Some(threshold)) => income > threshold,
        _ => false,
    }
}

/// Taxable-income threshold for Form 8995, by tax year and filing status.
///
/// Temporary: the 2025 values come from the Form 8995 instructions. These
/// move to tax-year configuration in a later step. `None` means no threshold
/// is known for the year, so no alert is shown.
fn taxable_income_threshold(
    tax_year: Option<i32>,
    is_joint: bool,
) -> Option<Decimal> {
    match (tax_year, is_joint) {
        (Some(2025), true) => Some(Decimal::new(394_600, 0)),
        (Some(2025), false) => Some(Decimal::new(197_300, 0)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;

    use super::*;

    #[test]
    fn sum_of_no_entries_is_zero() {
        assert_eq!(
            sum_trade_values(Vec::<Option<Decimal>>::new()),
            Decimal::ZERO
        );
    }

    #[test]
    fn blank_entries_count_as_zero() {
        let values = vec![None, Some(dec!(100.50)), None];
        assert_eq!(sum_trade_values(values), dec!(100.50));
    }

    #[test]
    fn negative_entries_reduce_the_total() {
        let values = vec![Some(dec!(250)), Some(dec!(-75))];
        assert_eq!(sum_trade_values(values), dec!(175));
    }

    #[test]
    fn entries_are_summed_without_rounding() {
        let values = vec![Some(dec!(1.10)), Some(dec!(2.20)), Some(dec!(3.30))];
        assert_eq!(sum_trade_values(values), dec!(6.60));
    }

    #[test]
    fn to_cents_rounds_halves_away_from_zero() {
        assert_eq!(to_cents(dec!(0.125)), dec!(0.13));
        assert_eq!(to_cents(dec!(0.124)), dec!(0.12));
        assert_eq!(to_cents(dec!(-0.125)), dec!(-0.13));
    }

    #[test]
    fn taxable_income_is_agi_minus_deduction() {
        assert_eq!(
            taxable_income_before_qbi(Some(dec!(120000)), Some(dec!(20000))),
            Some(dec!(100000))
        );
    }

    #[test]
    fn taxable_income_needs_both_values() {
        assert_eq!(taxable_income_before_qbi(None, Some(dec!(20000))), None);
        assert_eq!(taxable_income_before_qbi(Some(dec!(120000)), None), None);
    }

    #[test]
    fn compute_lines_sets_every_line_when_all_inputs_are_entered() {
        let inputs = QbiInputs {
            trade_total: dec!(70000.50),
            taxable_income: Some(dec!(80000.25)),
            line_12: dec!(550.75),
            ..QbiInputs::default()
        };
        assert_eq!(
            compute_lines(inputs),
            QbiLines {
                line_2: dec!(70000.50),
                line_4: dec!(70000.50),
                line_5: dec!(14000.10),
                line_8: Decimal::ZERO,
                line_9: Decimal::ZERO,
                line_10: dec!(14000.10),
                line_11: Some(dec!(80000.25)),
                line_13: Some(dec!(79449.50)),
                line_14: Some(dec!(15889.90)),
                line_15: dec!(14000.10),
                line_16: Decimal::ZERO,
                line_17: Decimal::ZERO,
            }
        );
    }

    #[test]
    fn reit_and_ptp_amounts_add_to_line_10() {
        let inputs = QbiInputs {
            trade_total: dec!(10000),
            line_6: dec!(5000),
            line_7: dec!(-1000),
            taxable_income: Some(dec!(100000)),
            ..QbiInputs::default()
        };
        assert_eq!(
            compute_lines(inputs),
            QbiLines {
                line_2: dec!(10000),
                line_4: dec!(10000),
                line_5: dec!(2000),
                line_8: dec!(4000),
                line_9: dec!(800),
                line_10: dec!(2800),
                line_11: Some(dec!(100000)),
                line_13: Some(dec!(100000)),
                line_14: Some(dec!(20000)),
                line_15: dec!(2800),
                line_16: Decimal::ZERO,
                line_17: Decimal::ZERO,
            }
        );
    }

    #[test]
    fn loss_carryforwards_zero_current_year_amounts() {
        let inputs = QbiInputs {
            trade_total: dec!(1000),
            line_3: dec!(-3000),
            line_6: dec!(-400),
            taxable_income: Some(dec!(10000)),
            ..QbiInputs::default()
        };
        assert_eq!(
            compute_lines(inputs),
            QbiLines {
                line_2: dec!(1000),
                line_4: Decimal::ZERO,
                line_5: Decimal::ZERO,
                line_8: Decimal::ZERO,
                line_9: Decimal::ZERO,
                line_10: Decimal::ZERO,
                line_11: Some(dec!(10000)),
                line_13: Some(dec!(10000)),
                line_14: Some(dec!(2000)),
                line_15: Decimal::ZERO,
                line_16: dec!(-2000),
                line_17: dec!(-400),
            }
        );
    }

    #[test]
    fn limitation_caps_line_15_at_line_14() {
        let inputs = QbiInputs {
            trade_total: dec!(100000),
            taxable_income: Some(dec!(10000)),
            ..QbiInputs::default()
        };
        assert_eq!(
            compute_lines(inputs),
            QbiLines {
                line_2: dec!(100000),
                line_4: dec!(100000),
                line_5: dec!(20000),
                line_8: Decimal::ZERO,
                line_9: Decimal::ZERO,
                line_10: dec!(20000),
                line_11: Some(dec!(10000)),
                line_13: Some(dec!(10000)),
                line_14: Some(dec!(2000)),
                line_15: dec!(2000),
                line_16: Decimal::ZERO,
                line_17: Decimal::ZERO,
            }
        );
    }

    #[test]
    fn missing_taxable_income_leaves_limitation_blank() {
        let inputs = QbiInputs {
            trade_total: dec!(100000),
            ..QbiInputs::default()
        };
        assert_eq!(
            compute_lines(inputs),
            QbiLines {
                line_2: dec!(100000),
                line_4: dec!(100000),
                line_5: dec!(20000),
                line_8: Decimal::ZERO,
                line_9: Decimal::ZERO,
                line_10: dec!(20000),
                line_11: None,
                line_13: None,
                line_14: None,
                line_15: Decimal::ZERO,
                line_16: Decimal::ZERO,
                line_17: Decimal::ZERO,
            }
        );
    }

    #[test]
    fn exceeds_threshold_only_above_the_limit() {
        assert!(exceeds_threshold(Some(dec!(197301)), Some(dec!(197300))));
        assert!(!exceeds_threshold(Some(dec!(197300)), Some(dec!(197300))));
        assert!(!exceeds_threshold(Some(dec!(197299)), Some(dec!(197300))));
    }

    #[test]
    fn exceeds_threshold_is_false_without_both_values() {
        assert!(!exceeds_threshold(None, Some(dec!(197300))));
        assert!(!exceeds_threshold(Some(dec!(500000)), None));
    }

    #[test]
    fn single_threshold_for_2025() {
        assert_eq!(
            taxable_income_threshold(Some(2025), false),
            Some(dec!(197300))
        );
    }

    #[test]
    fn joint_threshold_for_2025() {
        assert_eq!(
            taxable_income_threshold(Some(2025), true),
            Some(dec!(394600))
        );
    }

    #[test]
    fn no_threshold_for_unknown_year() {
        assert_eq!(taxable_income_threshold(Some(2030), false), None);
        assert_eq!(taxable_income_threshold(None, true), None);
    }
}
