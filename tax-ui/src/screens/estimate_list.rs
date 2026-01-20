use crate::app::{MessageType, TaxApp};
use egui::Ui;

pub struct EstimateListScreen;

impl EstimateListScreen {
    pub fn show(app: &mut TaxApp, ui: &mut Ui) {
        ui.heading("Saved Estimates");
        ui.separator();

        if app.saved_estimates.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(50.0);
                ui.label("No saved estimates found.");
                ui.add_space(20.0);

                if ui.button("Create New Estimate").clicked() {
                    app.current_screen = crate::app::Screen::Main;
                }

                ui.add_space(20.0);

                // Demo: Add some fake data for UI testing
                if ui.button("Load Demo Data").clicked() {
                    app.saved_estimates = create_demo_estimates();
                    app.show_message("Loaded demo estimates", MessageType::Info);
                }
            });
        } else {
            // Table of saved estimates
            egui::ScrollArea::vertical().show(ui, |ui| {
                egui::Grid::new("estimates_table")
                    .num_columns(5)
                    .striped(true)
                    .spacing([20.0, 8.0])
                    .show(ui, |ui| {
                        // Header
                        ui.strong("ID");
                        ui.strong("Tax Year");
                        ui.strong("AGI");
                        ui.strong("Total Tax");
                        ui.strong("Actions");
                        ui.end_row();

                        let estimates = app.saved_estimates.clone();
                        for estimate in &estimates {
                            ui.label(estimate.id.to_string());
                            ui.label(estimate.tax_year.to_string());
                            ui.label(format!("${:.2}", estimate.expected_agi));
                            ui.label(
                                estimate
                                    .calculated_total_tax
                                    .map(|t| format!("${t:.2}"))
                                    .unwrap_or_else(|| "—".to_string()),
                            );

                            ui.horizontal(|ui| {
                                if ui.small_button("Load").clicked() {
                                    load_estimate_into_form(app, estimate);
                                    app.current_screen = crate::app::Screen::Main;
                                    app.show_message(
                                        format!("Loaded estimate #{}", estimate.id),
                                        MessageType::Success,
                                    );
                                }
                                if ui.small_button("🗑").clicked() {
                                    // TODO: Implement delete
                                    app.show_message("Delete not yet implemented", MessageType::Info);
                                }
                            });
                            ui.end_row();
                        }
                    });
            });
        }
    }
}

fn load_estimate_into_form(app: &mut TaxApp, estimate: &tax_core::models::TaxEstimate) {
    app.form.tax_year = estimate.tax_year.to_string();
    app.form.expected_agi = estimate.expected_agi.to_string();
    app.form.expected_deduction = estimate.expected_deduction.to_string();
    app.form.expected_qbi_deduction = estimate
        .expected_qbi_deduction
        .map(|d| d.to_string())
        .unwrap_or_default();
    app.form.expected_amt = estimate
        .expected_amt
        .map(|d| d.to_string())
        .unwrap_or_default();
    app.form.expected_credits = estimate
        .expected_credits
        .map(|d| d.to_string())
        .unwrap_or_default();
    app.form.expected_other_taxes = estimate
        .expected_other_taxes
        .map(|d| d.to_string())
        .unwrap_or_default();
    app.form.expected_withholding = estimate
        .expected_withholding
        .map(|d| d.to_string())
        .unwrap_or_default();
    app.form.prior_year_tax = estimate
        .prior_year_tax
        .map(|d| d.to_string())
        .unwrap_or_default();
    app.form.se_income = estimate
        .se_income
        .map(|d| d.to_string())
        .unwrap_or_default();
    app.form.expected_crp_payments = estimate
        .expected_crp_payments
        .map(|d| d.to_string())
        .unwrap_or_default();
    app.form.expected_wages = estimate
        .expected_wages
        .map(|d| d.to_string())
        .unwrap_or_default();

    app.selected_estimate_id = Some(estimate.id);

    // Load calculated results too
    app.results.se_tax = estimate.calculated_se_tax;
    app.results.total_tax = estimate.calculated_total_tax;
    app.results.required_payment = estimate.calculated_required_payment;
    app.results.quarterly_payment = estimate
        .calculated_required_payment
        .map(|p| p / rust_decimal::Decimal::from(4));
}

fn create_demo_estimates() -> Vec<tax_core::models::TaxEstimate> {
    use chrono::Utc;
    use rust_decimal_macros::dec;

    vec![
        tax_core::models::TaxEstimate {
            id: 1,
            tax_year: 2025,
            filing_status_id: 1,
            expected_agi: dec!(75000),
            expected_deduction: dec!(14600),
            expected_qbi_deduction: None,
            expected_amt: None,
            expected_credits: None,
            expected_other_taxes: None,
            expected_withholding: Some(dec!(8000)),
            prior_year_tax: Some(dec!(9500)),
            se_income: None,
            expected_crp_payments: None,
            expected_wages: None,
            calculated_se_tax: None,
            calculated_total_tax: Some(dec!(9800)),
            calculated_required_payment: Some(dec!(1800)),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        },
        tax_core::models::TaxEstimate {
            id: 2,
            tax_year: 2025,
            filing_status_id: 2,
            expected_agi: dec!(150000),
            expected_deduction: dec!(29200),
            expected_qbi_deduction: Some(dec!(5000)),
            expected_amt: None,
            expected_credits: Some(dec!(2000)),
            expected_other_taxes: None,
            expected_withholding: Some(dec!(20000)),
            prior_year_tax: Some(dec!(22000)),
            se_income: Some(dec!(50000)),
            expected_crp_payments: None,
            expected_wages: Some(dec!(100000)),
            calculated_se_tax: Some(dec!(7065)),
            calculated_total_tax: Some(dec!(25000)),
            calculated_required_payment: Some(dec!(5000)),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        },
    ]
}
