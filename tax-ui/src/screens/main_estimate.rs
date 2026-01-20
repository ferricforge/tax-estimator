use crate::app::{FilingStatus, TaxApp};
use crate::widgets::currency_field;
use egui::Ui;

pub struct MainEstimateScreen;

impl MainEstimateScreen {
    pub fn show(app: &mut TaxApp, ui: &mut Ui) {
        ui.heading("Tax Estimate - 1040-ES Worksheet");
        ui.separator();

        egui::ScrollArea::vertical().show(ui, |ui| {
            // Basic Information Section
            ui.group(|ui| {
                ui.heading("Basic Information");
                ui.horizontal(|ui| {
                    ui.label("Tax Year:");
                    ui.text_edit_singleline(&mut app.form.tax_year);
                });

                ui.horizontal(|ui| {
                    ui.label("Filing Status:");
                    egui::ComboBox::from_id_salt("filing_status")
                        .selected_text(app.form.filing_status.label())
                        .show_ui(ui, |ui| {
                            for status in FilingStatus::all() {
                                ui.selectable_value(
                                    &mut app.form.filing_status,
                                    *status,
                                    status.label(),
                                );
                            }
                        });
                });
            });

            ui.add_space(10.0);

            // Income Section
            ui.group(|ui| {
                ui.heading("Income & Deductions");

                currency_field(ui, "Expected AGI:", &mut app.form.expected_agi);
                currency_field(ui, "Expected Deduction:", &mut app.form.expected_deduction);
                currency_field(
                    ui,
                    "QBI Deduction (optional):",
                    &mut app.form.expected_qbi_deduction,
                );
            });

            ui.add_space(10.0);

            // Tax Adjustments Section
            ui.group(|ui| {
                ui.heading("Tax Adjustments");

                currency_field(
                    ui,
                    "Alternative Minimum Tax:",
                    &mut app.form.expected_amt,
                );
                currency_field(ui, "Expected Credits:", &mut app.form.expected_credits);
                currency_field(ui, "Other Taxes:", &mut app.form.expected_other_taxes);
            });

            ui.add_space(10.0);

            // Withholding & Prior Year Section
            ui.group(|ui| {
                ui.heading("Withholding & Prior Year");

                currency_field(
                    ui,
                    "Expected Withholding:",
                    &mut app.form.expected_withholding,
                );
                currency_field(ui, "Prior Year Tax:", &mut app.form.prior_year_tax);
            });

            // Validation Errors
            if !app.form.errors.is_empty() {
                ui.add_space(10.0);
                ui.group(|ui| {
                    ui.colored_label(egui::Color32::RED, "Validation Errors:");
                    for error in &app.form.errors {
                        ui.colored_label(egui::Color32::RED, format!("  • {error}"));
                    }
                });
            }

            ui.add_space(20.0);

            // Action Buttons
            ui.horizontal(|ui| {
                if ui.button("Calculate Estimate").clicked() {
                    app.calculate();
                }

                if ui.button("Clear Form").clicked() {
                    app.form = crate::app::EstimateForm::new();
                    app.results = crate::app::CalculationResults::default();
                }
            });

            // Results Section
            if app.results.total_tax.is_some() {
                ui.add_space(20.0);
                ui.group(|ui| {
                    ui.heading("Calculation Results");

                    egui::Grid::new("results_grid")
                        .num_columns(2)
                        .spacing([20.0, 8.0])
                        .show(ui, |ui| {
                            if let Some(se_tax) = app.results.se_tax {
                                ui.label("Self-Employment Tax:");
                                ui.label(format!("${se_tax:.2}"));
                                ui.end_row();
                            }

                            if let Some(total) = app.results.total_tax {
                                ui.label("Total Estimated Tax:");
                                ui.strong(format!("${total:.2}"));
                                ui.end_row();
                            }

                            if let Some(required) = app.results.required_payment {
                                ui.label("Required Annual Payment:");
                                ui.strong(format!("${required:.2}"));
                                ui.end_row();
                            }

                            if let Some(quarterly) = app.results.quarterly_payment {
                                ui.label("Quarterly Payment:");
                                ui.heading(format!("${quarterly:.2}"));
                                ui.end_row();
                            }
                        });
                });
            }
        });
    }
}
