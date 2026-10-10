//! Form 8995 view: the simplified QBI deduction computation.
//!
//! Two models are held here. `model` holds the values the estimate saves, and
//! changes only when the user chooses Apply to Estimate. `draft` holds the
//! lines computed from what is currently in the inputs, and drives the screen.
//! Closing the dialog leaves the inputs and the draft in place, but does not
//! change what gets saved.

use gpui::{
    App, Context, Div, Entity, EventEmitter, InteractiveElement as _, IntoElement, ParentElement,
    Render, SharedString, StatefulInteractiveElement as _, Styled, TextAlign, Window, div, px,
};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::{ActiveTheme, h_flex, v_flex};
use rust_decimal::Decimal;
use tax_core::Qbi;
use tax_core::calculations::taxable_income_before_qbi;
use tax_core::validation::{
    QbiField, QbiRawInputs, QbiRawTrade, QbiWorksheetInputs, QbiWorksheetValidator, TradeColumn,
    Validated, Worksheet,
};

use crate::components::{
    ErrorDialog, FieldVisibility, confirm_clear, field_messages, form_error_banner, make_button,
    make_carryforward_display_row_with_help, make_decimal_input, make_display_row,
    make_display_row_with_help, make_header_row, make_help_slot, make_input_row_fixed_with_help,
    make_text_input, set_input_value, set_optional_decimal_input, visible_issues,
};
use crate::estimate::qbi_deduction_estimate;
use crate::instructions::{UiInstructionField, help_for_field};
use crate::models::{QBI_TRADE_ROW_LABELS, QbiWorksheetModel};

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

/// Fields outside the line 1 table, in form order.
const LINE_FIELDS: [QbiField; 4] = [
    QbiField::QbiLossCarryforward,
    QbiField::ReitPtpIncome,
    QbiField::ReitPtpLossCarryforward,
    QbiField::NetCapitalGain,
];

/// Key that remembers the "Don't show this again" choice for the QBI form.
const CLEAR_CONFIRM_KEY: &str = "qbi-form-clear";

/// Explains what clearing the form does to the saved estimate.
const CLEAR_CONFIRM_MESSAGE: &str = "Clearing this form removes its Form 8995 entries from the \
    estimate. The next time you save the estimate, the saved entries are deleted from the \
    database.";

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
    /// The saved Form 8995 values. Changes reach it only through Apply to
    /// Estimate, loading a saved record, or clearing the form.
    model: QbiWorksheetModel,
    /// The lines computed from the values currently in the inputs.
    draft: QbiWorksheetModel,
    /// Result of the most recent validation pass over every field.
    validated: Validated<QbiWorksheetInputs, QbiField>,
    /// Which fields show their messages.
    visibility: FieldVisibility<QbiField>,
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

        let mut form = Self {
            trade_rows,
            line_3_carryforward,
            line_6_reit_ptp,
            line_7_reit_ptp_carryforward,
            line_12_capital_gain_dividends,
            model: QbiWorksheetModel::default(),
            draft: QbiWorksheetModel::default(),
            validated: Validated::default(),
            visibility: FieldVisibility::default(),
        };
        form.watch_inputs(cx);
        form.refresh(cx);
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
        self.update_context(tax_year, is_joint, expected_agi, expected_deduction, cx);
        self.check_threshold(window, cx);
    }

    /// Like [`Self::set_context`], but without the threshold alert. Used
    /// before saving, so the saved line 11 and the lines figured from it match
    /// the estimate being saved.
    pub fn update_context(
        &mut self,
        tax_year: Option<i32>,
        is_joint: bool,
        expected_agi: Option<Decimal>,
        expected_deduction: Option<Decimal>,
        cx: &mut Context<Self>,
    ) {
        let income = taxable_income_before_qbi(expected_agi, expected_deduction);
        for model in [&mut self.model, &mut self.draft] {
            model.set_filing_context(tax_year, is_joint, income);
        }
        compute(&mut self.model);
        compute(&mut self.draft);
        cx.notify();
    }

    /// The saved Form 8995 values. This is what the estimate persists.
    pub fn model(&self) -> &QbiWorksheetModel {
        &self.model
    }

    /// Returns `true` when the current inputs carry no blocking message.
    ///
    /// This describes the inputs, not the saved values. Apply to Estimate
    /// refuses to commit while it returns `false`.
    pub fn is_valid(&self) -> bool {
        self.validated.is_valid()
    }

    /// Fills the inputs from a saved record and recalculates the lines. The
    /// values from the estimate are kept.
    pub fn populate_from_qbi(
        &mut self,
        qbi: &Qbi,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let row_count = self.trade_rows.len();
        if qbi.input.businesses.len() > row_count {
            tracing::warn!(
                estimate_id = qbi.tax_estimate_id,
                stored = qbi.input.businesses.len(),
                shown = row_count,
                "Form 8995 has more line 1 rows than the form shows; the rest are not loaded"
            );
        }
        self.model.populate_from_qbi(qbi);
        self.draft = self.model.clone();
        self.visibility.reset();
        self.write_inputs(window, cx);
        self.refresh(cx);
    }

    /// Empties every value the user enters, clears the saved entries and the
    /// draft, and clears the messages. The values from the estimate are kept.
    pub fn clear(
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
        self.draft.clear_entries();
        self.visibility.reset();
        self.refresh(cx);
    }

    /// Subscribes to every input so an edit marks its field as touched and
    /// refreshes the draft.
    fn watch_inputs(
        &self,
        cx: &mut Context<Self>,
    ) {
        for field in self.fields() {
            if let Some(input) = self.input_for(field) {
                watch(input.clone(), field, cx);
            }
        }
    }

    /// Every editable field, in form order.
    fn fields(&self) -> Vec<QbiField> {
        let mut fields: Vec<QbiField> = Vec::new();

        for row in 0..self.trade_rows.len() {
            fields.push(trade_field(row, TradeColumn::Name));
            fields.push(trade_field(row, TradeColumn::TaxpayerId));
            fields.push(trade_field(row, TradeColumn::QbiOrLoss));
        }
        fields.extend(LINE_FIELDS);
        fields
    }

    /// Returns the input that holds a field, if the form shows it.
    fn input_for(
        &self,
        field: QbiField,
    ) -> Option<&Entity<InputState>> {
        match field {
            QbiField::Trade { row, column } => {
                let trade = self.trade_rows.get(row)?;
                Some(match column {
                    TradeColumn::Name => &trade.name,
                    TradeColumn::TaxpayerId => &trade.taxpayer_id,
                    TradeColumn::QbiOrLoss => &trade.qbi,
                })
            }
            QbiField::QbiLossCarryforward => Some(&self.line_3_carryforward),
            QbiField::ReitPtpIncome => Some(&self.line_6_reit_ptp),
            QbiField::ReitPtpLossCarryforward => Some(&self.line_7_reit_ptp_carryforward),
            QbiField::NetCapitalGain => Some(&self.line_12_capital_gain_dividends),
        }
    }

    /// Validates every field and stores the result.
    fn validate_inputs(
        &mut self,
        cx: &App,
    ) {
        let cells: Vec<SharedString> = self
            .trade_rows
            .iter()
            .flat_map(|trade| {
                [
                    trade.name.read(cx).value(),
                    trade.taxpayer_id.read(cx).value(),
                    trade.qbi.read(cx).value(),
                ]
            })
            .collect();
        let trades: Vec<QbiRawTrade<'_>> = cells
            .as_chunks::<3>()
            .0
            .iter()
            .map(|cell| QbiRawTrade {
                name: cell[0].as_str(),
                taxpayer_id: cell[1].as_str(),
                qbi_or_loss: cell[2].as_str(),
            })
            .collect();

        let line_3 = self.line_3_carryforward.read(cx).value();
        let line_6 = self.line_6_reit_ptp.read(cx).value();
        let line_7 = self.line_7_reit_ptp_carryforward.read(cx).value();
        let line_12 = self.line_12_capital_gain_dividends.read(cx).value();

        let raw = QbiRawInputs {
            trades: &trades,
            qbi_loss_carryforward: line_3.as_str(),
            reit_ptp_income: line_6.as_str(),
            reit_ptp_loss_carryforward: line_7.as_str(),
            net_capital_gain: line_12.as_str(),
        };

        self.validated = QbiWorksheetValidator::validate(raw);
    }

    /// Copies the validated values into the draft. Text is trimmed and invalid
    /// amounts are empty, so the draft holds only normalized values.
    fn apply_validated(&mut self) {
        let inputs = &self.validated.inputs;
        let draft = &mut self.draft;

        for (entry, trade) in draft
            .line_1_trades_or_businesses
            .iter_mut()
            .zip(&inputs.trades)
        {
            entry.name = trade.name.clone();
            entry.taxpayer_id = trade.taxpayer_id.clone();
            entry.qbi_or_loss = trade.qbi_or_loss;
        }
        draft.line_3_qbi_loss_carryforward = inputs.qbi_loss_carryforward;
        draft.line_6_reit_ptp_income = inputs.reit_ptp_income;
        draft.line_7_reit_ptp_loss_carryforward = inputs.reit_ptp_loss_carryforward;
        draft.line_12_net_capital_gain = inputs.net_capital_gain;
    }

    /// Validates the inputs, updates the draft from them, recomputes the
    /// draft's lines, and re-renders. The saved model is not changed.
    fn refresh(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        self.validate_inputs(cx);
        self.apply_validated();
        compute(&mut self.draft);
        cx.notify();
    }

    /// Saves the draft as the form's values and sends line 15 to the owning
    /// window, so it can fill the estimate's QBI deduction field. Nothing is
    /// saved or sent while a field carries an error.
    fn apply_deduction(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        self.visibility.submit();

        if !self.is_valid() {
            tracing::warn!(
                errors = self.validated.report.error_count(),
                "Form 8995 inputs failed validation; the deduction was not applied"
            );
            cx.notify();
            return;
        }

        self.model = self.draft.clone();
        let deduction = self.model.line_15_qbi_deduction.unwrap_or_default();
        cx.emit(QbiFormEvent::ApplyQbiDeduction(deduction));
    }

    /// Copies the values from the saved model into the inputs.
    fn write_inputs(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let model = &self.model;
        for (trade, row) in model
            .line_1_trades_or_businesses
            .iter()
            .zip(&self.trade_rows)
        {
            set_input_value(&row.name, trade.name.clone(), window, cx);
            set_input_value(&row.taxpayer_id, trade.taxpayer_id.clone(), window, cx);
            set_optional_decimal_input(&row.qbi, trade.qbi_or_loss, window, cx);
        }
        set_optional_decimal_input(
            &self.line_3_carryforward,
            model.line_3_qbi_loss_carryforward,
            window,
            cx,
        );
        set_optional_decimal_input(
            &self.line_6_reit_ptp,
            model.line_6_reit_ptp_income,
            window,
            cx,
        );
        set_optional_decimal_input(
            &self.line_7_reit_ptp_carryforward,
            model.line_7_reit_ptp_loss_carryforward,
            window,
            cx,
        );
        set_optional_decimal_input(
            &self.line_12_capital_gain_dividends,
            model.line_12_net_capital_gain,
            window,
            cx,
        );
    }

    /// Shows an alert when taxable income is above the threshold. The inline
    /// warning is drawn from the draft, which `refresh` has already updated
    /// and marked for re-rendering.
    fn check_threshold(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.draft.above_threshold {
            let message = self.threshold_warning();
            ErrorDialog::show("Taxable income above threshold", &[message], window, cx);
        }
    }

    fn threshold_warning(&self) -> String {
        let threshold = self.draft.taxable_income_threshold.unwrap_or_default();
        format!("Taxable income is above the ${threshold:.0} threshold. Use Form 8995-A instead.")
    }

    /// Renders the messages for one field, when its visibility allows them.
    fn messages_for(
        &self,
        field: QbiField,
        cx: &App,
    ) -> Option<impl IntoElement> {
        let issues = visible_issues(&self.validated.report, &self.visibility, field);
        field_messages(issues, cx)
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
        if !self.draft.above_threshold {
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

    fn render_trade_table(
        &self,
        cx: &App,
    ) -> Div {
        let line_1_help = help_for_field(UiInstructionField::QbiLine1, self.draft.tax_year);
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
                .enumerate()
                .map(|(row, (inputs, index))| {
                    let name = self.messages_for(trade_field(row, TradeColumn::Name), cx);
                    let taxpayer_id =
                        self.messages_for(trade_field(row, TradeColumn::TaxpayerId), cx);
                    let qbi = self.messages_for(trade_field(row, TradeColumn::QbiOrLoss), cx);

                    v_flex()
                        .gap_1()
                        .child(make_trade_row(index, inputs))
                        .children(name)
                        .children(taxpayer_id)
                        .children(qbi)
                }),
        )
    }

    fn render_lines(
        &self,
        cx: &App,
    ) -> Div {
        let model = &self.draft;
        let year = model.tax_year;
        v_flex()
            .gap_2()
            .child(make_display_row_with_help(
                "2. Total qualified business income or (loss):",
                model.line_2_total_qbi_or_loss,
                help_for_field(UiInstructionField::QbiLine2, year),
            ))
            .child(
                v_flex()
                    .gap_1()
                    .child(make_input_row_fixed_with_help(
                        &self.line_3_carryforward,
                        "3. QBI net (loss) carryforward from prior year: $",
                        help_for_field(UiInstructionField::QbiLine3, year),
                    ))
                    .children(self.messages_for(QbiField::QbiLossCarryforward, cx)),
            )
            .child(make_display_row_with_help(
                "4. Total qualified business income (2 + 3):",
                model.line_4_total_qbi,
                help_for_field(UiInstructionField::QbiLine4, year),
            ))
            .child(make_display_row(
                "5. QBI component (4 × 20%):",
                model.line_5_qbi_component,
            ))
            .child(
                v_flex()
                    .gap_1()
                    .child(make_input_row_fixed_with_help(
                        &self.line_6_reit_ptp,
                        "6. Qualified REIT dividends and PTP income or (loss): $",
                        help_for_field(UiInstructionField::QbiLine6, year),
                    ))
                    .children(self.messages_for(QbiField::ReitPtpIncome, cx)),
            )
            .child(
                v_flex()
                    .gap_1()
                    .child(make_input_row_fixed_with_help(
                        &self.line_7_reit_ptp_carryforward,
                        "7. Qualified REIT and PTP (loss) carryforward: $",
                        help_for_field(UiInstructionField::QbiLine7, year),
                    ))
                    .children(self.messages_for(QbiField::ReitPtpLossCarryforward, cx)),
            )
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
            .child(
                v_flex()
                    .gap_1()
                    .child(make_input_row_fixed_with_help(
                        &self.line_12_capital_gain_dividends,
                        "12. Net capital gain plus qualified dividends: $",
                        help_for_field(UiInstructionField::QbiLine12, year),
                    ))
                    .children(self.messages_for(QbiField::NetCapitalGain, cx)),
            )
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
        let clear_target = this.clone();
        let show_banner = self.visibility.is_submitted() && !self.is_valid();

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
                    .child(self.render_trade_table(cx))
                    .child(self.render_lines(cx))
                    .children(show_banner.then(|| {
                        form_error_banner("Correct the fields marked above before applying.", cx)
                    }))
                    .child(
                        h_flex()
                            .gap_2()
                            .justify_end()
                            .mt_4()
                            .child(make_button(
                                "qbi_clear",
                                "Clear",
                                true,
                                move |_ev, window, app_cx| {
                                    let this = clear_target.clone();
                                    confirm_clear(
                                        CLEAR_CONFIRM_KEY,
                                        CLEAR_CONFIRM_MESSAGE,
                                        move |window, app| {
                                            this.update(app, |form, cx| {
                                                form.clear(window, cx);
                                            });
                                        },
                                        window,
                                        app_cx,
                                    );
                                },
                            ))
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

/// Computes the Form 8995 lines from the entries and context held in `model`.
fn compute(model: &mut QbiWorksheetModel) {
    let input = model.to_worksheet_input();
    match qbi_deduction_estimate(model.worksheet_config(), &input) {
        Ok(result) => model.from_worksheet_result(&result),
        Err(e) => tracing::warn!(%e, "QBI deduction calculation failed"),
    }
}

/// Builds the key for one cell of the line 1 table.
fn trade_field(
    row: usize,
    column: TradeColumn,
) -> QbiField {
    QbiField::Trade { row, column }
}

/// Refreshes the form, and marks the field as touched, when the user edits
/// its input. Focus and blur events are ignored so tabbing through a field
/// does not show its messages.
fn watch(
    input: Entity<InputState>,
    field: QbiField,
    cx: &mut Context<QbiForm>,
) {
    cx.subscribe(&input, move |this, _input, event, cx| {
        if let InputEvent::Change = event {
            this.visibility.touch(field);
            this.refresh(cx);
        }
    })
    .detach();
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
