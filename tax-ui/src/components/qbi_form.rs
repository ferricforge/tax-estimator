//! Form 8995 view: the simplified QBI deduction computation.
//!
//! The values entered here and the computed lines are held in a
//! [`QbiWorksheetModel`]. Line 11 comes from the estimate form. The other
//! computed lines are figured by `tax-core` on each change.

use gpui::{
    App, Context, Div, Entity, EventEmitter, InteractiveElement as _, IntoElement, ParentElement,
    Render, StatefulInteractiveElement as _, Styled, TextAlign, Window, div, px,
};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::{ActiveTheme, h_flex, v_flex};
use rust_decimal::Decimal;
use tax_core::calculations::taxable_income_before_qbi;

use crate::components::{
    ErrorDialog, make_button, make_carryforward_display_row_with_help, make_decimal_input,
    make_display_row, make_display_row_with_help, make_header_row, make_help_slot,
    make_input_row_fixed_with_help, make_text_input, set_input_value,
};
use crate::estimate::qbi_deduction_estimate;
use crate::instructions::{UiInstructionField, help_for_field};
use crate::models::{QBI_TRADE_ROW_LABELS, QbiWorksheetModel};
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

/// Events the QBI form sends to the window that owns it.
pub enum QbiFormEvent {
    /// The user chose to use line 15 as the estimate's QBI deduction.
    ApplyQbiDeduction(Decimal),
}

/// Inputs for one row of the line 1 table.
struct TradeRow {
    name: Entity<InputState>,
    taxpayer_id: Entity<InputState>,
    qbi: Entity<InputState>,
}

pub struct QbiForm {
    /// Line 1: one set of inputs per trade, business, or aggregation row.
    trade_rows: Vec<TradeRow>,
    /// Line 3: QBI net loss carryforward from the prior year.
    line_3_carryforward: Entity<InputState>,
    /// Line 6: qualified REIT dividends and PTP income or loss.
    line_6_reit_ptp: Entity<InputState>,
    /// Line 7: qualified REIT dividends and PTP loss carryforward.
    line_7_reit_ptp_carryforward: Entity<InputState>,
    /// Line 12: net capital gain increased by qualified dividends.
    line_12_capital_gain_dividends: Entity<InputState>,
    /// Form 8995 values, both entered and computed.
    model: QbiWorksheetModel,
}

impl EventEmitter<QbiFormEvent> for QbiForm {}

impl QbiForm {
    pub fn new(
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let trade_rows: Vec<TradeRow> = QBI_TRADE_ROW_LABELS
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
        for row in &trade_rows {
            watched.push(row.name.clone());
            watched.push(row.taxpayer_id.clone());
            watched.push(row.qbi.clone());
        }
        for input in &watched {
            cx.subscribe(input, |this, _input, event, cx| {
                if let InputEvent::Change = event {
                    this.recalculate(cx);
                }
            })
            .detach();
        }

        let mut form = Self {
            trade_rows,
            line_3_carryforward,
            line_6_reit_ptp,
            line_7_reit_ptp_carryforward,
            line_12_capital_gain_dividends,
            model: QbiWorksheetModel::default(),
        };
        form.recalculate_model();
        form
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
        let income = taxable_income_before_qbi(expected_agi, expected_deduction);
        self.model.set_filing_context(tax_year, is_joint, income);
        self.recalculate(cx);
        self.check_threshold(window, cx);
    }

    /// Empties every value the user enters and recalculates the lines. The
    /// values from the estimate are kept.
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
        self.model.clear_entries();
        self.recalculate(cx);
    }

    /// Sends line 15 to the owning window so it can fill the estimate's QBI
    /// deduction field.
    fn apply_deduction(
        &self,
        cx: &mut Context<Self>,
    ) {
        let deduction = self.model.line_15_qbi_deduction.unwrap_or_default();
        cx.emit(QbiFormEvent::ApplyQbiDeduction(deduction));
    }

    /// Reads the inputs into the model, recomputes every computed line, and
    /// re-renders.
    fn recalculate(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        self.read_inputs(cx);
        self.recalculate_model();
        cx.notify();
    }

    /// Copies the entered values into the model.
    fn read_inputs(
        &mut self,
        cx: &App,
    ) {
        let trades = &mut self.model.line_1_trades_or_businesses;
        for (trade, row) in trades.iter_mut().zip(&self.trade_rows) {
            trade.name = row.name.read(cx).value().to_string();
            trade.taxpayer_id = row.taxpayer_id.read(cx).value().to_string();
            trade.qbi_or_loss = read_optional(&row.qbi, cx);
        }
        let model = &mut self.model;
        model.line_3_qbi_loss_carryforward = read_optional(&self.line_3_carryforward, cx);
        model.line_6_reit_ptp_income = read_optional(&self.line_6_reit_ptp, cx);
        model.line_7_reit_ptp_loss_carryforward =
            read_optional(&self.line_7_reit_ptp_carryforward, cx);
        model.line_12_net_capital_gain = read_optional(&self.line_12_capital_gain_dividends, cx);
    }

    /// Computes the Form 8995 lines from the values in the model.
    fn recalculate_model(&mut self) {
        let input = self.model.to_worksheet_input();
        match qbi_deduction_estimate(self.model.worksheet_config(), &input) {
            Ok(result) => self.model.from_worksheet_result(&result),
            Err(e) => tracing::warn!(%e, "QBI deduction calculation failed"),
        }
    }

    /// Shows an alert when taxable income is above the threshold. The inline
    /// warning is drawn from the model, which `recalculate` has already
    /// updated and marked for re-rendering.
    fn check_threshold(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.model.above_threshold {
            let message = self.threshold_warning();
            ErrorDialog::show("Taxable income above threshold", &[message], window, cx);
        }
    }

    fn threshold_warning(&self) -> String {
        let threshold = self.model.taxable_income_threshold.unwrap_or_default();
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
        if !self.model.above_threshold {
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
        let line_1_help = help_for_field(UiInstructionField::QbiLine1, self.model.tax_year);
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
                .zip(QBI_TRADE_ROW_LABELS)
                .map(|(row, index)| make_trade_row(index, row)),
        )
    }

    fn render_lines(&self) -> Div {
        let model = &self.model;
        let year = model.tax_year;
        v_flex()
            .gap_2()
            .child(make_display_row_with_help(
                "2. Total qualified business income or (loss):",
                model.line_2_total_qbi_or_loss,
                help_for_field(UiInstructionField::QbiLine2, year),
            ))
            .child(make_input_row_fixed_with_help(
                &self.line_3_carryforward,
                "3. QBI net (loss) carryforward from prior year: $",
                help_for_field(UiInstructionField::QbiLine3, year),
            ))
            .child(make_display_row_with_help(
                "4. Total qualified business income (2 + 3):",
                model.line_4_total_qbi,
                help_for_field(UiInstructionField::QbiLine4, year),
            ))
            .child(make_display_row(
                "5. QBI component (4 × 20%):",
                model.line_5_qbi_component,
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
                model.line_8_total_reit_ptp_income,
                help_for_field(UiInstructionField::QbiLine8, year),
            ))
            .child(make_display_row(
                "9. REIT and PTP component (8 × 20%):",
                model.line_9_reit_ptp_component,
            ))
            .child(make_display_row(
                "10. QBI deduction before the income limitation (5 + 9):",
                model.line_10_deduction_before_income_limitation,
            ))
            .child(make_display_row_with_help(
                "11. Taxable income before QBI deduction:",
                model.line_11_taxable_income_before_qbi,
                help_for_field(UiInstructionField::QbiLine11, year),
            ))
            .child(make_input_row_fixed_with_help(
                &self.line_12_capital_gain_dividends,
                "12. Net capital gain plus qualified dividends: $",
                help_for_field(UiInstructionField::QbiLine12, year),
            ))
            .child(make_display_row(
                "13. Subtract line 12 from line 11:",
                model.line_13_taxable_income_less_net_capital_gain,
            ))
            .child(make_display_row(
                "14. Income limitation (13 × 20%):",
                model.line_14_income_limitation,
            ))
            .child(make_display_row_with_help(
                "15. QBI deduction (smaller of line 10 or line 14):",
                model.line_15_qbi_deduction,
                help_for_field(UiInstructionField::QbiLine15, year),
            ))
            .child(make_carryforward_display_row_with_help(
                "16. Total qualified business (loss) carryforward:",
                model.line_16_total_qbi_loss_carryforward,
                help_for_field(UiInstructionField::QbiLine16, year),
            ))
            .child(make_carryforward_display_row_with_help(
                "17. Total qualified REIT and PTP (loss) carryforward:",
                model.line_17_total_reit_ptp_loss_carryforward,
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
