use crate::app::{get_available_tax_years, FilingStatus, TaxApp};
use egui::Ui;
use rust_decimal::Decimal;

pub struct MainEstimateScreen;

impl MainEstimateScreen {
    /// Consistent group width matching SE screen
    const GROUP_WIDTH: f32 = 500.0;
    /// Label column width for alignment
    const LABEL_WIDTH: f32 = 200.0;
    /// Currency input field width
    const INPUT_WIDTH: f32 = 120.0;

    pub fn show(app: &mut TaxApp, ui: &mut Ui) {
        ui.heading("Tax Estimate - 1040-ES Worksheet");
        ui.separator();

        egui::ScrollArea::vertical().show(ui, |ui| {
            let group_width = ui.available_width().min(Self::GROUP_WIDTH);

            // Basic Information Section
            ui.allocate_ui(egui::vec2(group_width, 0.0), |ui| {
                ui.group(|ui| {
                    ui.set_min_width(group_width - 20.0);
                    ui.heading("Basic Information");
                    ui.add_space(5.0);

                    egui::Grid::new("basic_info_grid")
                        .num_columns(2)
                        .spacing([10.0, 8.0])
                        .show(ui, |ui| {
                            // Tax Year dropdown
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.set_min_width(Self::LABEL_WIDTH);
                                    ui.label(egui::RichText::new("Tax Year:").strong());
                                },
                            );
                            egui::ComboBox::from_id_salt("tax_year")
                                .width(80.0)
                                .selected_text(app.form.tax_year.to_string())
                                .show_ui(ui, |ui| {
                                    for year in get_available_tax_years() {
                                        ui.selectable_value(
                                            &mut app.form.tax_year,
                                            year,
                                            year.to_string(),
                                        );
                                    }
                                });
                            ui.end_row();

                            // Filing Status dropdown
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.set_min_width(Self::LABEL_WIDTH);
                                    ui.label(egui::RichText::new("Filing Status:").strong());
                                },
                            );
                            egui::ComboBox::from_id_salt("filing_status")
                                .width(200.0)
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
                            ui.end_row();
                        });
                });
            });

            ui.add_space(10.0);

            // Income & Deductions Section
            ui.allocate_ui(egui::vec2(group_width, 0.0), |ui| {
                ui.group(|ui| {
                    ui.set_min_width(group_width - 20.0);
                    ui.heading("Income & Deductions");
                    ui.add_space(5.0);

                    Self::currency_grid(ui, "income_grid", |ui| {
                        Self::grid_currency_row(
                            ui,
                            "Expected AGI:",
                            &mut app.form.expected_agi,
                            true,
                        );
                        Self::grid_currency_row(
                            ui,
                            "Expected Deduction:",
                            &mut app.form.expected_deduction,
                            true,
                        );
                        Self::grid_currency_row(
                            ui,
                            "QBI Deduction:",
                            &mut app.form.expected_qbi_deduction,
                            false,
                        );
                    });
                });
            });

            ui.add_space(10.0);

            // Tax Adjustments Section
            ui.allocate_ui(egui::vec2(group_width, 0.0), |ui| {
                ui.group(|ui| {
                    ui.set_min_width(group_width - 20.0);
                    ui.heading("Tax Adjustments");
                    ui.add_space(5.0);

                    Self::currency_grid(ui, "adjustments_grid", |ui| {
                        Self::grid_currency_row(
                            ui,
                            "Alternative Minimum Tax:",
                            &mut app.form.expected_amt,
                            false,
                        );
                        Self::grid_currency_row(
                            ui,
                            "Expected Credits:",
                            &mut app.form.expected_credits,
                            false,
                        );
                        Self::grid_currency_row(
                            ui,
                            "Other Taxes:",
                            &mut app.form.expected_other_taxes,
                            false,
                        );
                    });
                });
            });

            ui.add_space(10.0);

            // Withholding & Prior Year Section
            ui.allocate_ui(egui::vec2(group_width, 0.0), |ui| {
                ui.group(|ui| {
                    ui.set_min_width(group_width - 20.0);
                    ui.heading("Withholding & Prior Year");
                    ui.add_space(5.0);

                    Self::currency_grid(ui, "withholding_grid", |ui| {
                        Self::grid_currency_row(
                            ui,
                            "Expected Withholding:",
                            &mut app.form.expected_withholding,
                            false,
                        );
                        Self::grid_currency_row(
                            ui,
                            "Prior Year Tax:",
                            &mut app.form.prior_year_tax,
                            false,
                        );
                    });
                });
            });

            // Validation Errors
            if !app.form.errors.is_empty() {
                ui.add_space(10.0);
                ui.allocate_ui(egui::vec2(group_width, 0.0), |ui| {
                    ui.group(|ui| {
                        ui.set_min_width(group_width - 20.0);
                        ui.colored_label(egui::Color32::RED, "Validation Errors:");
                        for error in &app.form.errors {
                            ui.colored_label(egui::Color32::RED, format!("  • {error}"));
                        }
                    });
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

                if app.form.se_income.is_empty() {
                    if ui.button("Add Self-Employment →").clicked() {
                        app.current_screen = crate::app::Screen::SelfEmployment;
                    }
                } else {
                    if ui.button("Edit Self-Employment →").clicked() {
                        app.current_screen = crate::app::Screen::SelfEmployment;
                    }
                }
            });

            // Results Section
            if app.results.total_tax.is_some() {
                ui.add_space(20.0);
                ui.allocate_ui(egui::vec2(group_width, 0.0), |ui| {
                    ui.group(|ui| {
                        ui.set_min_width(group_width - 20.0);
                        ui.heading("Calculation Results");
                        ui.add_space(5.0);

                        egui::Grid::new("results_grid")
                            .num_columns(2)
                            .spacing([40.0, 8.0])
                            .show(ui, |ui| {
                                if let Some(se_tax) = app.results.se_tax {
                                    ui.label("Self-Employment Tax:");
                                    ui.label(format!("${:.2}", se_tax));
                                    ui.end_row();
                                }

                                if let Some(total) = app.results.total_tax {
                                    ui.label("Total Estimated Tax:");
                                    ui.strong(format!("${:.2}", total));
                                    ui.end_row();
                                }

                                if let Some(required) = app.results.required_payment {
                                    ui.label("Required Annual Payment:");
                                    ui.strong(format!("${:.2}", required));
                                    ui.end_row();
                                }

                                if let Some(quarterly) = app.results.quarterly_payment {
                                    ui.label("Quarterly Payment:");
                                    ui.heading(format!("${:.2}", quarterly));
                                    ui.end_row();
                                }
                            });

                        // Payment due dates
                        ui.add_space(10.0);
                        ui.separator();
                        ui.add_space(5.0);
                        ui.label("Quarterly Payment Due Dates:");
                        ui.horizontal(|ui| {
                            ui.label(format!("Q1: Apr 15, {}", app.form.tax_year));
                            ui.separator();
                            ui.label(format!("Q2: Jun 15, {}", app.form.tax_year));
                            ui.separator();
                            ui.label(format!("Q3: Sep 15, {}", app.form.tax_year));
                            ui.separator();
                            ui.label(format!("Q4: Jan 15, {}", app.form.tax_year + 1));
                        });
                    });
                });
            }

            ui.add_space(20.0);
        });
    }

    /// Create a grid for currency input alignment
    fn currency_grid(ui: &mut Ui, id: &str, add_contents: impl FnOnce(&mut Ui)) {
        egui::Grid::new(id)
            .num_columns(3)
            .spacing([10.0, 8.0])
            .min_col_width(0.0)
            .show(ui, add_contents);
    }

    /// Add a row to a currency grid with consistent layout
    fn grid_currency_row(ui: &mut Ui, label: &str, value: &mut String, required: bool) {
        // Column 1: Label (fixed width for alignment)
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.set_min_width(Self::LABEL_WIDTH);
            if required {
                ui.label(egui::RichText::new(label).strong());
            } else {
                ui.label(label);
            }
        });

        // Column 2: Dollar sign
        ui.label("$");

        // Column 3: Input field (fixed width)
        ui.add(
            egui::TextEdit::singleline(value)
                .desired_width(Self::INPUT_WIDTH)
                .hint_text("0.00"),
        );

        ui.end_row();
    }
}